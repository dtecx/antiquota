//! Owns dictation: a recording becomes text through a fresh agy call on a Gemini account.
//! No session or conversation is involved; the recording is deleted afterwards.
use crate::{AppError, Service};
use antiquota_core::{AskRequest, Cancellation, rank_accounts};

/// Largest recording accepted, in bytes.
pub const RECORDING_LIMIT: usize = 25_000_000;
/// Seconds to wait for a busy Gemini account before giving up.
const LEASE_WAIT: f64 = 120.0;
const PROMPT: &str = "If it contains clear human speech, transcribe it verbatim in the language spoken, as plain text with normal punctuation, and reply with only that text. If there is no clear human speech (silence, noise, tones, music), reply with exactly NO_SPEECH. Never guess or invent words.";

/// The words heard, and the account that transcribed them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub text: String,
    pub slot: String,
}

impl Service {
    pub async fn transcribe(&self, data: &[u8], extension: &str) -> Result<Transcript, AppError> {
        if data.is_empty() || data.len() > RECORDING_LIMIT {
            return Err(AppError::Invalid(
                "record between a second and a few minutes".into(),
            ));
        }
        let (folder, path) = self.store.save_recording(data, extension)?;
        let result = self.transcribe_file(&folder, &path).await;
        self.store.discard_recording(&path);
        result
    }

    async fn transcribe_file(&self, folder: &str, path: &str) -> Result<Transcript, AppError> {
        // An account may be busy for a moment (a quota probe or a turn): wait for one, bounded so
        // the request never hangs.
        let started = self.clock.now();
        let lease = loop {
            let ranked = rank_accounts(
                &self.account_overview()?,
                "gemini",
                None,
                &self.defaults()?,
                &self.accounts.routes(1000)?,
                self.clock.now(),
            );
            if ranked.is_empty() {
                return Err(AppError::Invalid(
                    "no account has Gemini quota for dictation right now".into(),
                ));
            }
            let mut found = None;
            for name in &ranked {
                if let Some(lease) = self.accounts.lease_named(name)? {
                    found = Some(lease);
                    break;
                }
            }
            if let Some(lease) = found {
                break lease;
            }
            if self.clock.now() - started >= LEASE_WAIT {
                return Err(AppError::Invalid(
                    "every Gemini account is busy right now; try again in a moment".into(),
                ));
            }
            self.clock.wait(1.0, Cancellation::default()).await?;
        };
        let slot = lease.slot().clone();
        self.accounts.prepare(&slot)?;
        let answer = self
            .runner
            .ask(
                AskRequest {
                    // View only, and without skipping permissions: allowed to run tools, agy converted
                    // the audio with ffmpeg, deleted files and read other conversations, taking
                    // minutes and hundreds of thousands of tokens (W-010).
                    prompt: format!(
                        "Listen to the audio file {path} with your file viewer; do not run commands or open other files. {PROMPT}"
                    ),
                    // Flash Low invents words for non-speech; Flash High says NO_SPEECH.
                    model: antiquota_core::DEFAULT_MODEL.into(),
                    cwd: folder.into(),
                    slot: slot.clone(),
                    add_dirs: vec![folder.into()],
                    timeout: 180.0,
                },
                Cancellation::default(),
            )
            .await?;
        drop(lease);
        if answer.status != "SUCCESS" {
            return Err(AppError::Invalid(
                "agy could not transcribe the recording".into(),
            ));
        }
        let text = answer.response.trim();
        Ok(Transcript {
            text: if text.is_empty() || text.contains("NO_SPEECH") {
                String::new()
            } else {
                text.into()
            },
            slot: slot.name,
        })
    }
}
