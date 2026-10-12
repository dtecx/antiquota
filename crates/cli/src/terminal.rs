//! Owns terminal restoration, inert output and the small scrollback-preserving prompt editor.
use crossterm::{
    cursor::MoveToColumn,
    event::{
        DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode, size},
};
use std::io::{IsTerminal, Write, stdout};
use unicode_width::UnicodeWidthChar;

pub struct Terminal;
impl Terminal {
    pub fn open() -> anyhow::Result<Self> {
        anyhow::ensure!(
            std::io::stdin().is_terminal() && stdout().is_terminal(),
            "interactive mode needs a terminal; use antiquota run for raw agy arguments"
        );
        enable_raw_mode()?;
        let terminal = Self;
        execute!(stdout(), EnableBracketedPaste)?;
        Ok(terminal)
    }
    pub fn write(&self, text: &str) -> std::io::Result<()> {
        let text = safe(text).replace('\n', "\r\n");
        let mut out = stdout().lock();
        out.write_all(text.as_bytes())?;
        out.flush()
    }
    pub fn line(&self, text: &str) -> std::io::Result<()> {
        self.write(&format!("{text}\n"))
    }
    pub fn heading(&self, text: &str) -> std::io::Result<()> {
        let color = std::env::var_os("NO_COLOR").is_none();
        if color {
            write!(stdout(), "\x1b[33;1m")?;
        }
        self.write(text)?;
        if color {
            write!(stdout(), "\x1b[0m")?;
        }
        self.line("")
    }
    pub fn clear_input(&self) -> std::io::Result<()> {
        execute!(stdout(), MoveToColumn(0), Clear(ClearType::CurrentLine))
    }
    pub fn prompt(&self, editor: &Editor, label: &str) -> std::io::Result<()> {
        let width = size()?.0 as usize;
        let (text, column) = editor.view(label, width);
        self.clear_input()?;
        write!(stdout(), "{text}")?;
        execute!(stdout(), MoveToColumn(column as u16))?;
        stdout().flush()
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = execute!(stdout(), DisableBracketedPaste);
        let _ = disable_raw_mode();
        let _ = writeln!(stdout());
    }
}

/// Terminal controls from agent output, paths and pasted prompts never execute.
pub fn safe(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

#[derive(Debug, PartialEq)]
pub enum Input {
    Line(String),
    Cancel,
    Exit,
}
#[derive(Default)]
// shortcut: cursor movement uses Unicode scalars; add grapheme editing for compound emoji input.
pub struct Editor {
    text: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    history_at: Option<usize>,
    draft: String,
}
impl Editor {
    pub fn restore(&mut self, text: &str) {
        self.set(&safe(text));
    }
    pub fn seed(&mut self, text: &str) {
        self.history.push(safe(text));
    }
    pub fn reset(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.history_at = None;
        self.draft.clear();
    }
    fn set(&mut self, text: &str) {
        self.text = text.chars().collect();
        self.cursor = self.text.len();
    }
    pub fn event(&mut self, event: Event) -> Option<Input> {
        match event {
            Event::Paste(text) => {
                let chars: Vec<_> = safe(&text.replace("\r\n", "\n").replace('\r', "\n"))
                    .chars()
                    .collect();
                self.text
                    .splice(self.cursor..self.cursor, chars.iter().copied());
                self.cursor += chars.len();
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Char('c') if ctrl => {
                        self.reset();
                        return Some(Input::Cancel);
                    }
                    KeyCode::Char('d') if ctrl && self.text.is_empty() => return Some(Input::Exit),
                    KeyCode::Esc => {
                        self.reset();
                        return Some(Input::Cancel);
                    }
                    KeyCode::Enter
                        if self.text.last() == Some(&'\\') && self.cursor == self.text.len() =>
                    {
                        self.text.pop();
                        self.text.push('\n');
                    }
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        self.text.insert(self.cursor, '\n');
                        self.cursor += 1;
                    }
                    KeyCode::Char('j') if ctrl => {
                        self.text.insert(self.cursor, '\n');
                        self.cursor += 1;
                    }
                    KeyCode::Enter => {
                        let text: String = self.text.iter().collect();
                        if !text.trim().is_empty() && self.history.last() != Some(&text) {
                            self.history.push(text.clone());
                        }
                        self.reset();
                        return Some(Input::Line(text));
                    }
                    KeyCode::Char('a') if ctrl => self.cursor = 0,
                    KeyCode::Char('e') if ctrl => self.cursor = self.text.len(),
                    KeyCode::Char('u') if ctrl => {
                        self.text.drain(..self.cursor);
                        self.cursor = 0;
                    }
                    KeyCode::Char('k') if ctrl => {
                        self.text.truncate(self.cursor);
                    }
                    KeyCode::Char('w') if ctrl => {
                        let end = self.cursor;
                        while self.cursor > 0 && self.text[self.cursor - 1].is_whitespace() {
                            self.cursor -= 1;
                        }
                        while self.cursor > 0 && !self.text[self.cursor - 1].is_whitespace() {
                            self.cursor -= 1;
                        }
                        self.text.drain(self.cursor..end);
                    }
                    KeyCode::Home => self.cursor = 0,
                    KeyCode::End => self.cursor = self.text.len(),
                    KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
                    KeyCode::Right => self.cursor = (self.cursor + 1).min(self.text.len()),
                    KeyCode::Backspace if self.cursor > 0 => {
                        self.cursor -= 1;
                        self.text.remove(self.cursor);
                    }
                    KeyCode::Delete if self.cursor < self.text.len() => {
                        self.text.remove(self.cursor);
                    }
                    KeyCode::Char('d') if ctrl && self.cursor < self.text.len() => {
                        self.text.remove(self.cursor);
                    }
                    KeyCode::Up if !self.history.is_empty() => {
                        if self.history_at.is_none() {
                            self.draft = self.text.iter().collect();
                        }
                        let at = self
                            .history_at
                            .unwrap_or(self.history.len())
                            .saturating_sub(1);
                        self.history_at = Some(at);
                        self.set(&self.history[at].clone());
                    }
                    KeyCode::Down if self.history_at.is_some() => {
                        let at = self.history_at.unwrap() + 1;
                        if at == self.history.len() {
                            self.history_at = None;
                            self.set(&self.draft.clone());
                        } else {
                            self.history_at = Some(at);
                            self.set(&self.history[at].clone());
                        }
                    }
                    KeyCode::Tab => {
                        let text: String = self.text.iter().collect();
                        let choices: Vec<_> = crate::commands::NAMES
                            .iter()
                            .filter(|name| name.starts_with(&text))
                            .collect();
                        if choices.len() == 1 {
                            self.set(&format!("{} ", choices[0]));
                        }
                    }
                    KeyCode::Char(c) if !ctrl && !c.is_control() => {
                        self.text.insert(self.cursor, c);
                        self.cursor += 1;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        None
    }
    fn view(&self, label: &str, width: usize) -> (String, usize) {
        if width < 3 {
            return (String::new(), 0);
        }
        let cells = |c: char| c.width().unwrap_or(0);
        let visible: Vec<char> = self
            .text
            .iter()
            .map(|&c| {
                if c == '\n' {
                    '↵'
                } else if c == '\t' {
                    ' '
                } else {
                    c
                }
            })
            .collect();
        let label: String = safe(label).chars().take(width / 2).collect();
        let prefix: usize = label.chars().map(cells).sum();
        let available = width.saturating_sub(prefix + 2);
        let mut start = 0;
        while visible[start..self.cursor]
            .iter()
            .copied()
            .map(cells)
            .sum::<usize>()
            > available
        {
            start += 1;
        }
        let mut text = label;
        text.push(if start > 0 { '‹' } else { ' ' });
        let mut used = 0;
        for &c in &visible[start..] {
            let next = cells(c);
            if used + next > available {
                break;
            }
            text.push(c);
            used += next;
        }
        let column = prefix
            + 1
            + visible[start..self.cursor]
                .iter()
                .copied()
                .map(cells)
                .sum::<usize>();
        (text, column)
    }
}

#[cfg(test)]
mod tests;
