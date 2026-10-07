//! Owns typed command dispatch; session and turn modules own the behaviors.
use crate::{AppError, Service};
use antiquota_core::{Command, Session};
use std::sync::Arc;
pub enum CommandResult {
    Session(Box<Session>),
    Sent(String),
    Done,
}
impl Service {
    pub async fn execute(self: &Arc<Self>, command: Command) -> Result<CommandResult, AppError> {
        match command {
            Command::Create {
                cwd,
                title,
                model,
                mode,
            } => Ok(CommandResult::Session(Box::new(
                self.create(&cwd, &title, &model, mode)?,
            ))),
            Command::Patch { session, fields } => Ok(CommandResult::Session(Box::new(
                self.patch(&session, &fields)?,
            ))),
            Command::Send {
                session,
                text,
                attachments,
                kind,
            } => Ok(CommandResult::Sent(
                self.send(&session, text, attachments, kind)?.into(),
            )),
            Command::Stop { session } => {
                self.stop(&session)?;
                Ok(CommandResult::Done)
            }
            Command::Delete { session } => {
                self.delete(&session).await?;
                Ok(CommandResult::Done)
            }
            Command::SetAwake { .. } => Err(AppError::Missing("keep-awake feature pending".into())),
        }
    }
}
