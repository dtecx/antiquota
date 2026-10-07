//! Owns one Terminal panel shell per session: start or reuse, input, resize and kill.
use crate::{AppError, Service};
use antiquota_core::{Terminal, TerminalFeed, TerminalInfo};
use std::sync::Arc;

impl Service {
    fn terminal(&self, id: &str) -> Option<Arc<dyn Terminal>> {
        self.terms
            .lock()
            .ok()
            .and_then(|terms| terms.get(id).cloned())
    }

    /// Starts the session's shell, or reuses it while it lives unless `restart` asks for a new one.
    pub fn term_start(
        &self,
        id: &str,
        cols: u16,
        rows: u16,
        restart: bool,
    ) -> Result<TerminalInfo, AppError> {
        let cwd = self.get(id)?.0.cwd;
        let mut terms = self
            .terms
            .lock()
            .map_err(|_| AppError::Invalid("terminals are unavailable".into()))?;
        if let Some(existing) = terms.get(id) {
            if !restart && existing.info().alive {
                return Ok(existing.info());
            }
            existing.kill();
        }
        let terminal = self.terminals.spawn(&cwd, cols, rows)?;
        let info = terminal.info();
        terms.insert(id.into(), terminal);
        Ok(info)
    }

    pub fn term_input(&self, id: &str, data: &str) -> Result<(), AppError> {
        let terminal = self
            .terminal(id)
            .ok_or_else(|| AppError::Missing("no terminal".into()))?;
        Ok(terminal.write(data.as_bytes())?)
    }

    /// Resizes the session's shell; without one this does nothing.
    pub fn term_resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), AppError> {
        if let Some(terminal) = self.terminal(id) {
            terminal.resize(cols, rows)?;
        }
        Ok(())
    }

    /// Ends and forgets the session's shell; ending a missing one is harmless.
    pub fn term_kill(&self, id: &str) {
        let removed = self
            .terms
            .lock()
            .ok()
            .and_then(|mut terms| terms.remove(id));
        if let Some(terminal) = removed {
            terminal.kill();
        }
    }

    /// The shell's details, recent output and following output for one stream client.
    pub fn term_feed(&self, id: &str) -> Result<(TerminalInfo, TerminalFeed), AppError> {
        let terminal = self
            .terminal(id)
            .ok_or_else(|| AppError::Missing("no terminal for this session".into()))?;
        let feed = terminal.subscribe();
        Ok((terminal.info(), feed))
    }
}
