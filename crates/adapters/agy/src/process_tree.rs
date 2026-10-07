//! Owns safe process-group/job lifetime and bounded process completion.
use antiquota_core::PortError;
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use std::{process::ExitStatus, time::Duration};
use tokio::process::Command;
pub struct ProcessTree {
    pub child: Box<dyn ChildWrapper>,
    terminated: bool,
}
impl ProcessTree {
    pub fn spawn(command: Command) -> Result<Self, PortError> {
        let mut command = CommandWrap::from(command);
        command.wrap(KillOnDrop);
        #[cfg(unix)]
        command.wrap(process_wrap::tokio::ProcessGroup::leader());
        #[cfg(windows)]
        command.wrap(process_wrap::tokio::JobObject);
        let child = command
            .spawn()
            .map_err(|e| PortError::Unavailable(format!("cannot start agy: {e}")))?;
        Ok(Self {
            child,
            terminated: false,
        })
    }
    pub fn stop(&mut self) {
        if !self.terminated {
            if let Err(error) = self.child.start_kill() {
                tracing::debug!(%error,"runner tree already exited or could not terminate");
            } else {
                self.terminated = true;
            }
        }
    }
    pub fn status(&mut self) -> Result<Option<ExitStatus>, PortError> {
        self.child
            .try_wait()
            .map_err(|e| PortError::Unavailable(e.to_string()))
    }
    pub async fn finish(&mut self, status: Option<ExitStatus>) -> Result<ExitStatus, PortError> {
        self.stop();
        if let Some(status) = status {
            return Ok(status);
        }
        tokio::time::timeout(Duration::from_secs(2), self.child.wait())
            .await
            .map_err(|_| PortError::Unavailable("runner tree termination timed out".into()))?
            .map_err(|e| PortError::Unavailable(e.to_string()))
    }
}
impl Drop for ProcessTree {
    fn drop(&mut self) {
        self.stop();
    }
}
