//! Owns detached background jobs: own process group, output to a log file, survival across
//! engine restarts, and stopping the whole tree.
use antiquota_core::{JobRunner, PortError, PortFuture};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::process::Child;

/// How often an adopted job (started by an earlier engine) is checked.
const ADOPTED_POLL: Duration = Duration::from_secs(5);

#[derive(Default, Clone)]
pub struct Jobs {
    children: Arc<Mutex<BTreeMap<u32, Child>>>,
}

fn alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use nix::{errno::Errno, sys::signal::kill, unistd::Pid};
        let Ok(raw) = i32::try_from(pid) else {
            return false;
        };
        !matches!(kill(Pid::from_raw(raw), None), Err(Errno::ESRCH))
    }
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
}

impl JobRunner for Jobs {
    fn start(&self, cwd: &str, command: &str, log: &str) -> Result<u32, PortError> {
        let path = std::path::Path::new(log);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| PortError::Unavailable(format!("cannot create the job log: {e}")))?;
        }
        let out = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| PortError::Unavailable(format!("cannot open the job log: {e}")))?;
        let err = out
            .try_clone()
            .map_err(|e| PortError::Unavailable(e.to_string()))?;
        let mut process = crate::shell_command(command);
        process
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .env("NO_COLOR", "1")
            .env("PYTHONUNBUFFERED", "1")
            .kill_on_drop(false);
        // Its own process group: stopping the engine leaves the job running, Stop ends it all.
        #[cfg(unix)]
        process.process_group(0);
        #[cfg(windows)]
        process.creation_flags(0x0000_0200); // CREATE_NEW_PROCESS_GROUP
        let child = process
            .spawn()
            .map_err(|e| PortError::Unavailable(format!("cannot start the job: {e}")))?;
        let pid = child
            .id()
            .ok_or_else(|| PortError::Unavailable("the job ended at once".into()))?;
        self.children
            .lock()
            .map_err(|_| PortError::Unavailable("jobs are unavailable".into()))?
            .insert(pid, child);
        Ok(pid)
    }

    fn wait<'a>(&'a self, pid: u32) -> PortFuture<'a, Option<i32>> {
        Box::pin(async move {
            let child = self
                .children
                .lock()
                .map_err(|_| PortError::Unavailable("jobs are unavailable".into()))?
                .remove(&pid);
            if let Some(mut child) = child {
                let status = child
                    .wait()
                    .await
                    .map_err(|e| PortError::Unavailable(e.to_string()))?;
                return Ok(status.code());
            }
            while alive(pid) {
                tokio::time::sleep(ADOPTED_POLL).await;
            }
            Ok(None)
        })
    }

    fn stop(&self, pid: u32) -> Result<(), PortError> {
        #[cfg(unix)]
        {
            use nix::{sys::signal::Signal, unistd::Pid};
            let raw = i32::try_from(pid).map_err(|_| PortError::Invalid("bad pid".into()))?;
            let _ = nix::sys::signal::killpg(Pid::from_raw(raw), Signal::SIGTERM);
        }
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/T", "/F", "/PID", &pid.to_string()])
                .output();
        }
        Ok(())
    }

    fn read_log(&self, log: &str, max_bytes: usize) -> Result<String, PortError> {
        let Ok(mut file) = std::fs::File::open(log) else {
            return Ok(String::new());
        };
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        let skip = size.saturating_sub(max_bytes as u64);
        file.seek(SeekFrom::Start(skip))
            .map_err(|e| PortError::Unavailable(e.to_string()))?;
        let mut bytes = vec![];
        file.read_to_end(&mut bytes)
            .map_err(|e| PortError::Unavailable(e.to_string()))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn jobs_log_report_their_code_and_stop_as_a_tree() {
        let dir = std::env::temp_dir().join(format!("antiquota-jobs-{}", std::process::id()));
        let log = dir.join("a.log").to_string_lossy().into_owned();
        let jobs = Jobs::default();
        let pid = jobs
            .start(
                dir.parent().unwrap().to_str().unwrap(),
                "echo out; echo err >&2; exit 4",
                &log,
            )
            .unwrap();
        assert_eq!(jobs.wait(pid).await.unwrap(), Some(4));
        assert_eq!(jobs.read_log(&log, 1000).unwrap(), "out\nerr\n");
        assert_eq!(jobs.read_log(&log, 4).unwrap(), "err\n");
        let slow = dir.join("b.log").to_string_lossy().into_owned();
        let pid = jobs.start("/", "sleep 30 & sleep 30", &slow).unwrap();
        let other = Jobs::default();
        let adopted = tokio::spawn(async move { other.wait(pid).await });
        tokio::time::sleep(Duration::from_millis(200)).await;
        let started = std::time::Instant::now();
        jobs.stop(pid).unwrap();
        assert!(
            jobs.wait(pid).await.unwrap().is_none(),
            "terminated by a signal"
        );
        assert_eq!(adopted.await.unwrap().unwrap(), None);
        assert!(started.elapsed() < Duration::from_secs(8));
        assert_eq!(jobs.read_log("/nonexistent/log", 10).unwrap(), "");
        let env = dir.join("c.log").to_string_lossy().into_owned();
        let pid = jobs
            .start(
                "/",
                "printf '%s%s' \"$NO_COLOR\" \"$PYTHONUNBUFFERED\"; read x || echo no-stdin",
                &env,
            )
            .unwrap();
        assert_eq!(jobs.wait(pid).await.unwrap(), Some(0));
        assert_eq!(jobs.read_log(&env, 100).unwrap(), "11no-stdin\n");
        let _ = std::fs::remove_dir_all(dir);
    }
}
