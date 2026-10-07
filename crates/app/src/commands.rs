//! Owns typed command dispatch; session and turn modules own the behaviors.
use crate::{AppError, Service};
use antiquota_core::{AccountCommand, Command, Session};
use std::sync::Arc;
pub enum CommandResult {
    Session(Box<Session>),
    Sent(String),
    Done,
    Value(serde_json::Value),
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
            Command::Account { action } => {
                let data = match action {
                    AccountCommand::Probe => {
                        let service = self.clone();
                        tokio::spawn(async move {
                            if let Err(error) = service.refresh_accounts(0.0).await {
                                tracing::warn!(%error,"account refresh failed");
                            }
                        });
                        serde_json::json!({"ok":true})
                    }
                    AccountCommand::Check { name } => {
                        serde_json::to_value(self.check_account(&name).await?)
                            .map_err(|e| AppError::Invalid(e.to_string()))?
                    }
                    AccountCommand::Pause { name, on } => {
                        serde_json::to_value(self.pause_account(&name, on)?)
                            .map_err(|e| AppError::Invalid(e.to_string()))?
                    }
                    AccountCommand::Remove { name } => {
                        self.remove_account(&name)?;
                        let service = self.clone();
                        tokio::spawn(async move {
                            let _ = service.refresh_accounts(0.0).await;
                        });
                        serde_json::json!({"ok":true})
                    }
                    AccountCommand::Unwall { name } => {
                        self.unwall(&name)?;
                        serde_json::json!({"ok":true})
                    }
                    AccountCommand::LoginStart => {
                        let link = self.accounts.login_start().await?;
                        let service = self.clone();
                        let id = link.id.clone();
                        tokio::spawn(async move {
                            loop {
                                if service.background_cancel.is_cancelled() {
                                    break;
                                }
                                let Ok(state) = service.accounts.login_state(&id) else {
                                    break;
                                };
                                if state.state != antiquota_core::SignInState::Waiting {
                                    if matches!(
                                        state.state,
                                        antiquota_core::SignInState::Done
                                            | antiquota_core::SignInState::Verify
                                    ) {
                                        let _ = service.refresh_accounts(0.0).await;
                                    }
                                    break;
                                }
                                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                            }
                        });
                        serde_json::to_value(link).map_err(|e| AppError::Invalid(e.to_string()))?
                    }
                    AccountCommand::LoginSubmit { id, code } => {
                        serde_json::to_value(self.accounts.login_submit(&id, &code).await?)
                            .map_err(|e| AppError::Invalid(e.to_string()))?
                    }
                    AccountCommand::LoginCancel { id } => {
                        self.accounts.login_cancel(&id).await?;
                        serde_json::json!({"ok":true})
                    }
                    AccountCommand::Defaults { values } => {
                        serde_json::to_value(self.save_defaults(&values)?)
                            .map_err(|e| AppError::Invalid(e.to_string()))?
                    }
                };
                Ok(CommandResult::Value(data))
            }
            Command::ContinueTurn { .. } => Err(AppError::Invalid(
                "continuation is owned by the active turn".into(),
            )),
            Command::SetAwake { .. } => Err(AppError::Missing("keep-awake feature pending".into())),
        }
    }
}
