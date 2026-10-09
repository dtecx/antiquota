//! Owns how `antiquota goal` follows its session: it decodes the shared typed event stream,
//! writes the terminal log, and catches up from the saved session after the engine ends a
//! stream that fell behind, so no saved event is lost.
use antiquota_core::{Event, EventRecord, Session, StreamEvent};
use std::{collections::HashSet, io::Write};

/// One goal session's view of the shared event stream.
pub struct Follow {
    id: String,
    cwd: String,
    /// Timestamps of the transcript events shown; they are unique within a session.
    shown: HashSet<u64>,
    started: bool,
    /// The goal state from the newest session snapshot.
    pub state: String,
}

impl Follow {
    pub fn new(id: &str, cwd: &str) -> Self {
        Self {
            id: id.into(),
            cwd: cwd.into(),
            shown: HashSet::new(),
            started: false,
            state: "active".into(),
        }
    }

    /// Shows the complete frames in `buffer` and leaves a partial frame for the next chunk.
    /// Bytes are decoded per frame, so a character split between chunks stays whole.
    /// Returns true once the session has gone idle after working.
    pub fn frames(
        &mut self,
        out: &mut impl Write,
        warn: &mut impl Write,
        buffer: &mut Vec<u8>,
    ) -> bool {
        let mut done = false;
        while let Some(end) = buffer.windows(2).position(|pair| pair == b"\n\n") {
            let bytes: Vec<u8> = buffer.drain(..end + 2).collect();
            let message = String::from_utf8_lossy(&bytes);
            let Some(data) = message.lines().find_map(|l| l.strip_prefix("data: ")) else {
                continue;
            };
            match serde_json::from_str::<StreamEvent>(data) {
                Ok(event) if event.sid.as_deref() == Some(self.id.as_str()) => {
                    done |= self.record(out, &event.record, true);
                }
                Ok(_) => {}
                Err(error) => {
                    let _ = writeln!(warn, "antiquota goal: skipped an unreadable event: {error}");
                }
            }
        }
        done
    }

    /// Shows the saved events the stream did not show, then takes the saved state.
    pub fn catch_up(
        &mut self,
        out: &mut impl Write,
        meta: &Session,
        events: &[EventRecord],
    ) -> bool {
        for record in events {
            self.record(out, record, false);
        }
        self.meta(meta)
    }

    fn record(&mut self, out: &mut impl Write, record: &EventRecord, live: bool) -> bool {
        if let Event::Meta { meta } = &record.event {
            return self.meta(meta);
        }
        if record.event.is_transcript() && !self.shown.insert(record.ts.to_bits()) {
            return false;
        }
        show(out, &record.event, &self.cwd, live);
        false
    }

    fn meta(&mut self, meta: &Session) -> bool {
        self.state = meta
            .goal
            .as_ref()
            .map(|goal| goal.state.clone())
            .unwrap_or_default();
        if meta.status != "idle" {
            self.started = true;
            return false;
        }
        // A catch-up can miss the whole working stretch; an ended goal still finishes the run.
        self.started || (meta.goal.is_some() && self.state != "active")
    }
}

/// Writes one event to the terminal log. Live replies arrive as deltas, so a live reply only
/// ends its line; a reply from the saved session prints its whole text.
pub fn show(out: &mut impl Write, event: &Event, cwd: &str, live: bool) {
    let _ = match event {
        Event::Delta { text, .. } => write!(out, "{text}"),
        Event::Assistant { .. } if live => writeln!(out),
        Event::Assistant { text, .. } => writeln!(out, "{text}"),
        Event::User { text, kind, .. } if kind != "user" => {
            let body: String = text.chars().take(400).collect();
            writeln!(out, "\n>> {kind}: {body}")
        }
        Event::TurnStart { turn, slot, model } => {
            writeln!(out, "\n-- turn {turn} on {slot} ({model})")
        }
        Event::Tool {
            name,
            params,
            state,
            ..
        } if state != "running" => {
            let arg = ["CommandLine", "TargetFile", "AbsolutePath", "Query"]
                .iter()
                .find_map(|k| params[k].as_str())
                .unwrap_or("")
                .replace(&format!("{cwd}/"), "");
            let mark = if state == "error" { 'x' } else { '.' };
            let line: String = format!("   {mark} {name} {arg}")
                .chars()
                .take(200)
                .collect();
            writeln!(out, "{line}")
        }
        Event::TurnEnd { status, secs, .. } => writeln!(out, "   {status} in {secs:.1}s"),
        Event::Notice { kind, text, .. } => writeln!(out, "== {kind}: {text}"),
        _ => Ok(()),
    };
    let _ = out.flush();
}

#[cfg(test)]
mod tests;
