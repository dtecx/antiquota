//! Builds the handoff document from durable session records without a model call.
use crate::{Event, EventRecord, Session};
use std::collections::BTreeSet;

pub fn head(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}
pub fn tail(text: &str, count: usize) -> String {
    text.chars()
        .rev()
        .take(count)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}
pub fn handoff(meta: &Session, records: &[EventRecord], prompt: &str) -> String {
    format!(
        "You are taking over work from an earlier agent session whose conversation could not be resumed. Its handoff state follows; check the current files yourself before relying on it.\n\n{}\n\nContinue. Current request:\n{prompt}",
        document(meta, records)
    )
}
struct WorkTurn {
    number: u64,
    slot: String,
    status: String,
    edits: Vec<String>,
    commands: Vec<String>,
}
pub fn document(meta: &Session, records: &[EventRecord]) -> String {
    let mut slots = vec![];
    for r in records {
        if let Event::TurnStart { slot, .. } = &r.event
            && !slots.contains(slot)
        {
            slots.push(slot.clone());
        }
    }
    let mut out = vec![
        format!(
            "# Handoff: {}",
            if meta.title.is_empty() {
                "untitled session"
            } else {
                &meta.title
            }
        ),
        format!(
            "Folder: {}  ·  model {}  ·  {} turns  ·  accounts used: {}",
            meta.cwd,
            meta.model,
            meta.turns,
            if slots.is_empty() {
                "none yet".into()
            } else {
                slots.join(", ")
            }
        ),
    ];
    if let Some(g) = &meta.goal {
        out.extend([
            "".into(),
            "## Objective".into(),
            g.objective.clone(),
            format!(
                "State: {}, round {}/{}{}",
                g.state,
                g.round,
                g.max_rounds,
                if g.check.is_empty() {
                    String::new()
                } else {
                    format!(", check `{}`", g.check)
                }
            ),
        ]);
        let judged: Vec<_> = records
            .iter()
            .filter_map(|r| {
                if let Event::Notice { kind, text, data } = &r.event
                    && ["check", "verdict"].contains(&kind.as_str())
                {
                    Some((kind, text, data))
                } else {
                    None
                }
            })
            .collect();
        for (kind, text, data) in judged
            .into_iter()
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let mut line = format!("- {kind}: {}", head(text, 300));
            for item in data["missing"].as_array().into_iter().flatten().take(8) {
                line.push_str(&format!("\n  - {}", item.as_str().unwrap_or("")));
            }
            out.push(line);
        }
    }
    let asks: Vec<_> = records
        .iter()
        .filter_map(|r| {
            if let Event::User { text, kind, .. } = &r.event {
                Some((text, kind))
            } else {
                None
            }
        })
        .collect();
    if !asks.is_empty() {
        out.extend(["".into(), "## Requests (oldest first)".into()]);
        for (text, kind) in asks
            .into_iter()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            out.push(format!("- [{kind}] {}", head(text, 400).trim()));
        }
    }
    let mut turns: Vec<WorkTurn> = vec![];
    for r in records {
        match &r.event {
            Event::TurnStart { turn, slot, .. } => turns.push(WorkTurn {
                number: *turn,
                slot: slot.clone(),
                status: "?".into(),
                edits: vec![],
                commands: vec![],
            }),
            Event::Tool {
                name,
                params,
                diff,
                error,
                ..
            } if !turns.is_empty() => {
                let current = turns.last_mut().unwrap();
                if diff.as_ref().is_some_and(|d| !d.is_empty())
                    || [
                        "write_to_file",
                        "replace_file_content",
                        "multi_replace_file_content",
                        "sed_file",
                    ]
                    .contains(&name.as_str())
                {
                    let path = params["TargetFile"]
                        .as_str()
                        .or_else(|| params["File"].as_str())
                        .unwrap_or("?");
                    let rel = std::path::Path::new(path)
                        .strip_prefix(&meta.cwd)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| path.into());
                    if !current.edits.contains(&rel) {
                        current.edits.push(rel);
                    }
                } else if name == "run_command"
                    && let Some(cmd) = params["CommandLine"].as_str()
                {
                    current.commands.push(format!(
                        "{}{}",
                        head(cmd, 120),
                        if error
                            .as_ref()
                            .is_some_and(|v| !v.is_null() && v != &serde_json::json!(false))
                        {
                            " (failed)"
                        } else {
                            ""
                        }
                    ));
                }
            }
            Event::TurnEnd { status, .. } if !turns.is_empty() => {
                turns.last_mut().unwrap().status = status.clone()
            }
            _ => {}
        }
    }
    if !turns.is_empty() {
        out.extend(["".into(), "## Work log (latest turns)".into()]);
        for WorkTurn {
            number: n,
            slot,
            status,
            edits,
            commands: ran,
        } in turns
            .iter()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let mut line = format!("- #{n} {} {status}", slot.split('@').next().unwrap_or(slot));
            if !edits.is_empty() {
                line.push_str(&format!(
                    " · edited {}",
                    edits.iter().take(8).cloned().collect::<Vec<_>>().join(", ")
                ));
            }
            if !ran.is_empty() {
                line.push_str(&format!(
                    " · ran {}",
                    ran.iter()
                        .rev()
                        .take(4)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .map(|r| format!("`{r}`"))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
            out.push(line);
        }
    }
    let files: BTreeSet<_> = turns.iter().flat_map(|t| t.edits.iter()).collect();
    if !files.is_empty() {
        out.extend(["".into(), "## Files changed in this session".into()]);
        out.extend(files.into_iter().take(60).map(|f| format!("- {f}")));
    }
    if !meta.jobs.is_empty() {
        out.extend(["".into(), "## Background jobs".into()]);
        for j in meta
            .jobs
            .iter()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            out.push(format!(
                "- `{}`: {}, log {}",
                j["cmd"].as_str().unwrap_or(""),
                if j["ended"].as_f64().unwrap_or(0.0) == 0.0 {
                    "running".into()
                } else if j["stopped"] == true {
                    "stopped".into()
                } else {
                    format!("exit {}", j["code"])
                },
                j["log"].as_str().unwrap_or("")
            ));
        }
    }
    let problems: Vec<_> = records
        .iter()
        .rev()
        .take(80)
        .filter_map(|r| {
            if let Event::Notice { kind, text, .. } = &r.event
                && ["error", "denied"].contains(&kind.as_str())
            {
                Some((kind, text))
            } else {
                None
            }
        })
        .take(3)
        .collect();
    if !problems.is_empty() {
        out.extend(["".into(), "## Open problems".into()]);
        out.extend(
            problems
                .into_iter()
                .rev()
                .map(|(kind, text)| format!("- {kind}: {}", head(text, 300))),
        );
    }
    if let Some(text) = records.iter().rev().find_map(|r| {
        if let Event::Assistant { text, .. } = &r.event {
            Some(text)
        } else {
            None
        }
    }) {
        out.extend([
            "".into(),
            "## Last reply".into(),
            tail(text, 1500).trim().into(),
        ]);
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn record(value: serde_json::Value) -> EventRecord {
        serde_json::from_value(value).unwrap()
    }
    #[test]
    fn document_bounds_unicode_requests_turns_jobs_problems_and_last_reply() {
        let meta = Session { title: "Bounded handoff".into(), cwd: "/project".into(), turns: 14,
            goal: Some(serde_json::from_value(json!({"objective":"Keep working","check":"test","state":"running","round":3,"max_rounds":8})).unwrap()),
            jobs: (0..8).map(|n|json!({"cmd":format!("job{n}"),"ended":1,"code":0,"log":format!("log{n}")})).collect(), ..Default::default() };
        let mut records = vec![];
        for n in 0..14 {
            records.push(record(json!({"t":"user","ts":n,"turn":n,"text":format!("request{n}: {}", "界".repeat(500)),"kind":"user","attachments":[]})));
            records.push(record(
                json!({"t":"turn_start","ts":n,"turn":n,"slot":"one@example.com","model":"model"}),
            ));
            records.push(record(json!({"t":"tool","ts":n,"turn":n,"name":"sed_file","params":{"TargetFile":format!("/project/file{n}")},"state":"done","step":1})));
            records.push(record(
                json!({"t":"turn_end","ts":n,"turn":n,"status":"SUCCESS","usage":{},"slot":"one@example.com","secs":0}),
            ));
            records.push(record(
                json!({"t":"notice","ts":n,"kind":"error","text":format!("problem{n}"),"data":{}}),
            ));
        }
        records.push(record(json!({"t":"assistant","ts":15,"turn":14,"step":1,"text":format!("DROP{}TAIL","界".repeat(1600))})));
        for n in 0..4 {
            records.push(record(json!({"t":"notice","ts":20+n,"kind":if n%2==0 {"check"}else{"verdict"},"text":format!("verdict{n}"),"data":{"missing":["Missing proof"]}})));
        }
        for n in 0..65 {
            records.insert(2,record(json!({"t":"tool","ts":0,"turn":0,"step":1,"name":"write_to_file","state":"done","params":{"TargetFile":format!("/project/extra{n:02}")}})));
        }
        let text = document(&meta, &records);
        assert_eq!(
            text.split("## Files changed in this session\n")
                .nth(1)
                .unwrap()
                .split("\n\n## Background jobs")
                .next()
                .unwrap()
                .lines()
                .count(),
            60
        );
        assert!(!text.contains("verdict1"));
        assert!(text.contains("verdict2"));
        assert!(text.contains("Missing proof"));
        assert!(text.contains("## Objective\nKeep working"));
        assert!(!text.contains("request5:"));
        assert!(text.contains("request6:"));
        assert_eq!(
            text.lines().filter(|s| s.starts_with("- [user]")).count(),
            8
        );
        assert!(
            text.lines()
                .filter(|s| s.starts_with("- [user]"))
                .all(|s| s.chars().count() == 409)
        );
        assert!(!text.contains("- #1 "));
        assert!(text.contains("- #2 "));
        assert!(text.contains("edited file13"));
        assert!(!text.contains("`job1`"));
        assert!(text.contains("`job2`"));
        assert!(!text.contains("problem10"));
        assert!(text.contains("problem11"));
        assert!(!text.contains("DROP"));
        assert!(text.ends_with("TAIL"));
        assert_eq!(
            text.split("## Last reply\n")
                .last()
                .unwrap()
                .chars()
                .count(),
            1500
        );
        assert_eq!(head("界a", 1), "界");
        assert_eq!(tail("a界", 1), "界");
    }
}
