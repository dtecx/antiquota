//! Owns background jobs as data: the instruction agy gets, the request lines it may answer
//! with, intervals and daily times, job records and the messages a job sends back.
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::LazyLock};
use ts_rs::TS;

/// Sent with the first request of a fresh conversation, so long commands become jobs.
pub const JOBS_HINT: &str = "(antiquota) For a command that runs longer than about two minutes (training, long benchmarks, big test suites), do not run it and wait. End your reply with a line\nRUN_IN_BACKGROUND: <shell command>\nand optionally CHECK_EVERY: <minutes>m on the next line for progress check-ins. antiquota runs it in this folder, logs its output and sends you the log when it exits.";
/// Seconds between /loop prompts, at least.
pub const LOOP_MIN: u64 = 60;
/// Jobs kept in a session's record.
pub const JOBS_KEPT: usize = 20;
/// Characters of a job's log passed back to agy.
pub const JOB_TAIL: usize = 4000;
/// Characters of a job's log shown in the log view.
pub const LOG_VIEW: usize = 200_000;

static BG_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[ \t>*_]*RUN_IN_BACKGROUND:[*_]*[ \t]*`*(.+?)`*[ \t]*$").expect("valid pattern")
});
static EVERY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[ \t>*_]*CHECK_EVERY:[*_]*[ \t]*(\d+(?:\.\d+)?)[ \t]*([smh]?)")
        .expect("valid pattern")
});
static INTERVAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*(\d+(?:\.\d+)?)\s*([smh]?)\s*$").expect("valid pattern"));
static HHMM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([01]?\d|2[0-3]):([0-5]\d)$").expect("valid pattern"));
static ANSI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]").expect("valid pattern"));

/// "90s", "10m", "1.5h" or "15" (in `unit`) as whole seconds; 0 when it does not parse.
pub fn parse_interval(text: &str, unit: char) -> u64 {
    let Some(hit) = INTERVAL.captures(text) else {
        return 0;
    };
    let value: f64 = hit[1].parse().unwrap_or(0.0);
    let unit = hit
        .get(2)
        .and_then(|u| u.as_str().chars().next())
        .unwrap_or(unit)
        .to_ascii_lowercase();
    let scale = match unit {
        's' => 1.0,
        'h' => 3600.0,
        _ => 60.0,
    };
    (value * scale) as u64
}

/// A daily time "H:MM" or "HH:MM" from 00:00 to 23:59.
pub fn parse_hhmm(text: &str) -> Option<(u32, u32)> {
    let hit = HHMM.captures(text.trim())?;
    Some((hit[1].parse().ok()?, hit[2].parse().ok()?))
}

/// Background requests in a reply: `RUN_IN_BACKGROUND: <command>` lines outside fenced code
/// blocks, so an example in a code block never starts a job. A `<placeholder>` such as the one
/// in the instruction agy received is never a command.
pub fn background_requests(reply: &str) -> Vec<String> {
    let mut fenced = false;
    let mut found = vec![];
    for line in reply.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(hit) = BG_LINE.captures(line) {
            let command = hit[1].trim().to_owned();
            let placeholder = command.starts_with('<') && command.ends_with('>');
            if !command.is_empty() && !placeholder && !found.contains(&command) {
                found.push(command);
            }
        }
    }
    found
}

/// Seconds between progress check-ins requested with `CHECK_EVERY:`, 0 when absent.
pub fn check_every(reply: &str) -> u64 {
    reply
        .lines()
        .find_map(|line| EVERY_LINE.captures(line))
        .map(|hit| {
            let unit = hit.get(2).map_or("", |u| u.as_str());
            parse_interval(&format!("{}{}", &hit[1], unit), 'm')
        })
        .unwrap_or(0)
}

/// A job as stored in session metadata; unknown fields are kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Job {
    pub id: String,
    pub cmd: String,
    pub pid: Option<u32>,
    pub log: String,
    pub started: f64,
    pub ended: Option<f64>,
    pub code: Option<i32>,
    #[serde(default)]
    pub every: u64,
    #[serde(default = "yes")]
    pub notify: bool,
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub stopped: bool,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: BTreeMap<String, Value>,
}
fn yes() -> bool {
    true
}
impl Job {
    pub fn from_value(value: &Value) -> Option<Self> {
        serde_json::from_value(value.clone()).ok()
    }
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
    pub fn running(&self) -> bool {
        self.ended.is_none()
    }
}

/// "42s", "3m05s" or "2h07m".
pub fn took(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else {
        format!("{}h{:02}m", s / 3600, (s / 60) % 60)
    }
}

/// The last `count` characters of a log, without terminal colour codes.
pub fn clean_tail(text: &str, count: usize) -> String {
    let clean = ANSI.replace_all(text, "");
    let total = clean.chars().count();
    clean.chars().skip(total.saturating_sub(count)).collect()
}

/// The message agy gets when a job it should hear about ends.
pub fn job_result(job: &Job, log_tail: &str) -> String {
    let status = job
        .code
        .map_or_else(|| "finished".to_owned(), |c| format!("exit {c}"));
    format!(
        "Background job `{}` {status} after {}. End of its log ({}):\n```\n{log_tail}\n```\nAnalyze the result and continue.",
        job.cmd,
        took(job.ended.unwrap_or(job.started) - job.started),
        job.log
    )
}

/// The progress message sent every `every` seconds while a job runs and the session is idle.
pub fn job_check_in(job: &Job, now: f64, log_tail: &str) -> String {
    let pid = job.pid.map_or_else(|| "?".to_owned(), |p| p.to_string());
    format!(
        "Background job `{}` is still running ({}, pid {pid}). End of its log:\n```\n{log_tail}\n```\nIf it is going wrong, stop it (`kill {pid}`) and fix the cause; otherwise reply in one line and do not wait for it.",
        job.cmd,
        took(now - job.started)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intervals_and_times_follow_the_legacy_grammar() {
        assert_eq!(parse_interval("90s", 'm'), 90);
        assert_eq!(parse_interval("10m", 'm'), 600);
        assert_eq!(parse_interval("1.5H", 'm'), 5400);
        assert_eq!(parse_interval(" 15 ", 'm'), 900);
        assert_eq!(parse_interval("15", 's'), 15);
        for bad in ["", "invalid", "-5m", "5 minutes", "1e3"] {
            assert_eq!(parse_interval(bad, 'm'), 0, "{bad}");
        }
        assert_eq!(parse_hhmm("3:00"), Some((3, 0)));
        assert_eq!(parse_hhmm(" 23:59 "), Some((23, 59)));
        assert_eq!(parse_hhmm("00:00"), Some((0, 0)));
        for bad in ["24:00", "25:00", "12:60", "1200", "12:5"] {
            assert_eq!(parse_hhmm(bad), None, "{bad}");
        }
    }
    #[test]
    fn background_requests_skip_code_blocks_and_duplicates() {
        let reply = "Training needs a while.\nRUN_IN_BACKGROUND: `python train.py --epochs 3`\n**CHECK_EVERY:** 10m\n\nExample of the syntax:\n```\nRUN_IN_BACKGROUND: rm -rf /\n```\n> RUN_IN_BACKGROUND: make bench\nRUN_IN_BACKGROUND: make bench\ntext RUN_IN_BACKGROUND: inline is ignored";
        assert_eq!(
            background_requests(reply),
            ["python train.py --epochs 3", "make bench"]
        );
        assert_eq!(check_every(reply), 600);
        assert_eq!(check_every("CHECK_EVERY: 90s"), 90);
        assert_eq!(check_every("nothing"), 0);
        assert!(background_requests("RUN_IN_BACKGROUND:   ").is_empty());
        assert!(
            background_requests(JOBS_HINT).is_empty(),
            "the instruction itself starts nothing"
        );
    }
    #[test]
    fn job_records_keep_unknown_fields_and_messages_read_well() {
        let value = serde_json::json!({"id":"ab12cd34","cmd":"make","pid":7,"log":"/l","started":100.0,
            "ended":null,"code":null,"every":0,"notify":false,"by":"user","stopped":false,"extra":1});
        let job = Job::from_value(&value).unwrap();
        assert!(job.running() && !job.notify);
        assert_eq!(job.to_value(), value);
        let ended = Job {
            ended: Some(185.0),
            code: Some(2),
            ..job.clone()
        };
        assert_eq!(
            job_result(&ended, "boom"),
            "Background job `make` exit 2 after 1m25s. End of its log (/l):\n```\nboom\n```\nAnalyze the result and continue."
        );
        assert!(
            job_check_in(&job, 7300.0, "x")
                .starts_with("Background job `make` is still running (2h00m, pid 7).")
        );
        assert_eq!(took(42.0), "42s");
        assert_eq!(clean_tail("\x1b[31mred\x1b[0m and more", 8), "and more");
    }
}
