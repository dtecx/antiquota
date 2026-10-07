//! Owns bounded zero-token agy metadata calls and real quota-envelope parsing.
use crate::{FILE_TOKENS, FileAccounts};
use antiquota_core::{
    Failure, Model, PortError, PortFuture, ProbeResult, Quota, QuotaProbe, Slot, classify_error,
};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};
pub fn find_agy(home: &Path) -> Result<PathBuf, PortError> {
    if let Some(path) = std::env::var_os("ANTIQUOTA_AGY") {
        return Ok(path.into());
    }
    let mut dirs = vec![
        home.join(".local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Programs/agy"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    if let Some(path) = find_in_dirs(&dirs) {
        return Ok(path);
    }
    Err(PortError::Unavailable(
        "antiquota: real agy binary not found; set ANTIQUOTA_AGY".into(),
    ))
}
pub(crate) fn find_in_dirs(dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in dirs {
        for file in if cfg!(windows) {
            vec!["agy.exe", "agy.cmd", "agy.bat", "agy.com", "agy"]
        } else {
            vec!["agy"]
        } {
            let path = dir.join(file);
            if !path.is_file() {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if std::fs::metadata(&path).ok()?.permissions().mode() & 0o111 == 0 {
                    continue;
                }
            }
            return Some(path.canonicalize().unwrap_or(path));
        }
    }
    None
}
pub fn parse_quota(value: &Value) -> Result<Quota, PortError> {
    let invalid = || PortError::Invalid("unreadable quota panel".into());
    let groups = value["command"]["data"]["groups"]
        .as_array()
        .ok_or_else(invalid)?;
    let mut pools = BTreeMap::new();
    for group in groups {
        let name = group["name"].as_str().ok_or_else(invalid)?;
        let bucket = if name.starts_with("Gemini") {
            "gemini"
        } else {
            "cg"
        };
        let mut windows = BTreeMap::new();
        for window in group["buckets"].as_array().ok_or_else(invalid)? {
            let fraction = match window.get("remaining_fraction") {
                None | Some(Value::Null) => 0.0,
                Some(value) => value
                    .as_f64()
                    .or_else(|| value.as_str()?.parse().ok())
                    .ok_or_else(invalid)?,
            };
            if !fraction.is_finite() {
                return Err(invalid());
            }
            let reset = window["reset_time"]
                .as_str()
                .filter(|v| !v.is_empty())
                .map(|text| {
                    // https://docs.rs/chrono/0.4.45/chrono/struct.DateTime.html#method.parse_from_rfc3339
                    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
                        return Ok(
                            dt.timestamp() as f64 + f64::from(dt.timestamp_subsec_nanos()) / 1e9
                        );
                    }
                    let local = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f")
                        .map_err(|_| invalid())?;
                    Local
                        .from_local_datetime(&local)
                        .earliest()
                        .map(|dt| {
                            dt.timestamp() as f64 + f64::from(dt.timestamp_subsec_nanos()) / 1e9
                        })
                        .ok_or_else(invalid)
                })
                .transpose()?;
            windows.insert(
                window["window"].as_str().ok_or_else(invalid)?.into(),
                (fraction, reset),
            );
        }
        pools.insert(bucket.into(), windows);
    }
    Ok(Quota(pools))
}
pub fn google_url(text: &str, verification: bool) -> Option<String> {
    // Match only agy's own Google sign-in/verification origin.
    let prefix = if verification {
        "https://accounts.google.com/signin/continue"
    } else {
        "https://accounts.google.com/"
    };
    let start = text.find(prefix)?;
    Some(
        text[start..]
            .split(|c: char| {
                c.is_whitespace() || c == '"' || c == '\'' || c == '<' || c == '>' || c.is_control()
            })
            .next()?
            .trim_end_matches(['\u{1b}', '"'])
            .into(),
    )
}
struct Process {
    child: Box<dyn ChildWrapper>,
    stdout: tokio::task::JoinHandle<Result<Vec<u8>, PortError>>,
    stderr: tokio::task::JoinHandle<Result<Vec<u8>, PortError>>,
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        self.stdout.abort();
        self.stderr.abort();
    }
}
pub(crate) struct Captured {
    pub code: i32,
    pub out: String,
    pub err: String,
}
async fn read(reader: impl tokio::io::AsyncRead + Unpin, limit: u64) -> Result<Vec<u8>, PortError> {
    let mut output = vec![];
    reader
        .take(limit + 1)
        .read_to_end(&mut output)
        .await
        .map_err(|e| PortError::Unavailable(e.to_string()))?;
    if output.len() as u64 > limit {
        return Err(PortError::Invalid(
            "agy metadata output exceeds the limit".into(),
        ));
    }
    Ok(output)
}
impl FileAccounts {
    pub(crate) async fn capture(
        &self,
        slot: &Slot,
        args: &[String],
        timeout: Duration,
    ) -> Result<Captured, PortError> {
        let mut command = Command::new(&self.executable);
        command
            .args(args)
            .current_dir(&slot.home)
            .env("HOME", &slot.home)
            .env("USERPROFILE", &slot.home)
            .env("SSH_CONNECTION", FILE_TOKENS)
            .env("ANTIQUOTA_SLOT", &slot.name)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut wrapped = CommandWrap::from(command);
        wrapped.wrap(KillOnDrop);
        #[cfg(unix)]
        wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
        #[cfg(windows)]
        wrapped.wrap(process_wrap::tokio::JobObject);
        let mut child = wrapped
            .spawn()
            .map_err(|e| PortError::Unavailable(format!("cannot start agy metadata: {e}")))?;
        let stdout = child
            .stdout()
            .take()
            .ok_or_else(|| PortError::Unavailable("metadata stdout unavailable".into()))?;
        let stderr = child
            .stderr()
            .take()
            .ok_or_else(|| PortError::Unavailable("metadata stderr unavailable".into()))?;
        let mut process = Process {
            child,
            stdout: tokio::spawn(read(stdout, 1_000_000)),
            stderr: tokio::spawn(read(stderr, 65_536)),
        };
        let mut out = None;
        let mut err = None;
        let status = tokio::time::timeout(timeout, async {
            loop {
                if process.stdout.is_finished() && out.is_none() {
                    out = Some(
                        (&mut process.stdout)
                            .await
                            .map_err(|e| PortError::Unavailable(e.to_string()))??,
                    );
                }
                if process.stderr.is_finished() && err.is_none() {
                    err = Some(
                        (&mut process.stderr)
                            .await
                            .map_err(|e| PortError::Unavailable(e.to_string()))??,
                    );
                }
                if let Some(status) = process
                    .child
                    .try_wait()
                    .map_err(|e| PortError::Unavailable(e.to_string()))?
                {
                    break Ok::<_, PortError>(status);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|_| PortError::Unavailable("agy metadata timed out".into()))??;
        let _ = process.child.start_kill();
        let out = match out {
            Some(out) => out,
            None => tokio::time::timeout(Duration::from_millis(500), &mut process.stdout)
                .await
                .map_err(|_| PortError::Unavailable("metadata output did not close".into()))?
                .map_err(|e| PortError::Unavailable(e.to_string()))??,
        };
        let err = match err {
            Some(err) => err,
            None => tokio::time::timeout(Duration::from_millis(500), &mut process.stderr)
                .await
                .map_err(|_| PortError::Unavailable("metadata error output did not close".into()))?
                .map_err(|e| PortError::Unavailable(e.to_string()))??,
        };
        Ok(Captured {
            code: status.code().unwrap_or(-1),
            out: String::from_utf8_lossy(&out).into_owned(),
            err: String::from_utf8_lossy(&err).into_owned(),
        })
    }
    pub async fn identity(&self, slot: &Slot) -> Result<Option<String>, PortError> {
        self.sync(slot)?;
        let log = self
            .root
            .join("state/whoami")
            .join(format!("{}.log", slot.name));
        if let Some(parent) = log.parent() {
            std::fs::create_dir_all(parent).map_err(antiquota_store::io_error)?;
        }
        crate::files::unlink(&log)?;
        self.capture(
            slot,
            &[
                "-p".into(),
                "/quota".into(),
                "--output-format".into(),
                "json".into(),
                "--log-file".into(),
                log.to_string_lossy().into_owned(),
            ],
            Duration::from_secs(90),
        )
        .await?;
        let text = std::fs::read_to_string(log).unwrap_or_default();
        Ok(identity_from_log(&text))
    }
    pub async fn loaded(
        &self,
        slot: &Slot,
    ) -> Result<BTreeMap<String, Option<Vec<String>>>, PortError> {
        self.sync(slot)?;
        let mut loaded = BTreeMap::new();
        for (command, key) in [("/skills", "skills"), ("/hooks", "hooks")] {
            let response = self
                .capture(
                    slot,
                    &[
                        "-p".into(),
                        command.into(),
                        "--output-format".into(),
                        "json".into(),
                    ],
                    Duration::from_secs(90),
                )
                .await;
            let names = response
                .ok()
                .and_then(|r| serde_json::from_str::<Value>(&r.out).ok())
                .and_then(|v| {
                    v["command"]["data"][key].as_array().map(|items| {
                        items
                            .iter()
                            .filter_map(|v| v["name"].as_str().map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                });
            loaded.insert(
                key.into(),
                names.map(|mut n| {
                    n.sort();
                    n
                }),
            );
        }
        Ok(loaded)
    }
}
pub fn identity_from_log(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| {
            line.split_once("authenticated successfully as ")
                .map(|(_, tail)| {
                    tail.split_whitespace()
                        .next()
                        .unwrap_or("")
                        .trim_end_matches(['.', ',', ';'])
                        .to_owned()
                })
        })
        .rfind(|v| v.contains('@'))
}
impl QuotaProbe for FileAccounts {
    fn probe<'a>(&'a self, slot: &'a Slot) -> PortFuture<'a, ProbeResult> {
        Box::pin(async move {
            self.sync(slot)?;
            let result = self
                .capture(
                    slot,
                    &[
                        "-p".into(),
                        "/quota".into(),
                        "--output-format".into(),
                        "json".into(),
                    ],
                    Duration::from_secs(90),
                )
                .await;
            let Ok(result) = result else {
                return Ok(ProbeResult::Unreadable);
            };
            let errors = format!("{}{}", result.out, result.err);
            if let Some(url) = google_url(&errors, true)
                && errors.to_lowercase().contains("eligib")
            {
                return Ok(ProbeResult::Verify(url));
            }
            if result.code != 0 && classify_error(&errors) == Some(Failure::Auth) {
                return Ok(ProbeResult::SignedOut);
            }
            Ok(serde_json::from_str(&result.out)
                .ok()
                .and_then(|value| parse_quota(&value).ok())
                .map(ProbeResult::Quota)
                .unwrap_or(ProbeResult::Unreadable))
        })
    }
    fn models<'a>(&'a self, slot: &'a Slot) -> PortFuture<'a, Vec<Model>> {
        Box::pin(async move {
            self.sync(slot)?;
            let result = self
                .capture(slot, &["models".into()], Duration::from_secs(60))
                .await?;
            Ok(result
                .out
                .lines()
                .filter_map(|line| {
                    line.split_once('\t').map(|(id, label)| Model {
                        id: id.into(),
                        label: label.into(),
                    })
                })
                .collect())
        })
    }
}
