use super::*;
use crossterm::event::KeyEvent;
fn key(editor: &mut Editor, code: KeyCode, modifiers: KeyModifiers) -> Option<Input> {
    editor.event(Event::Key(KeyEvent::new(code, modifiers)))
}
fn type_text(editor: &mut Editor, text: &str) {
    for c in text.chars() {
        key(editor, KeyCode::Char(c), KeyModifiers::NONE);
    }
}
#[test]
fn editing_history_restores_drafts_and_never_sends_a_paste() {
    let mut e = Editor::default();
    type_text(&mut e, "old");
    assert_eq!(
        key(&mut e, KeyCode::Enter, KeyModifiers::NONE),
        Some(Input::Line("old".into()))
    );
    type_text(&mut e, "draft");
    key(&mut e, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(e.text.iter().collect::<String>(), "old");
    key(&mut e, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(e.text.iter().collect::<String>(), "draft");
    key(&mut e, KeyCode::Left, KeyModifiers::NONE);
    key(&mut e, KeyCode::Backspace, KeyModifiers::NONE);
    key(&mut e, KeyCode::Char('d'), KeyModifiers::NONE);
    assert_eq!(e.text.iter().collect::<String>(), "dradt");
    e.reset();
    assert_eq!(
        e.event(Event::Paste("你好\r\nλ\u{1b}[2J\u{7}".into())),
        None
    );
    assert_eq!(
        key(&mut e, KeyCode::Enter, KeyModifiers::NONE),
        Some(Input::Line("你好\nλ[2J".into()))
    );
}
#[test]
fn multiline_completion_and_control_keys_are_explicit() {
    let mut e = Editor::default();
    type_text(&mut e, "one\\");
    key(&mut e, KeyCode::Enter, KeyModifiers::NONE);
    type_text(&mut e, "two");
    key(&mut e, KeyCode::Char('j'), KeyModifiers::CONTROL);
    type_text(&mut e, "three");
    assert_eq!(
        key(&mut e, KeyCode::Enter, KeyModifiers::NONE),
        Some(Input::Line("one\ntwo\nthree".into()))
    );
    type_text(&mut e, "/sta");
    key(&mut e, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(e.text.iter().collect::<String>(), "/status ");
    assert_eq!(
        key(&mut e, KeyCode::Char('c'), KeyModifiers::CONTROL),
        Some(Input::Cancel)
    );
    assert!(e.text.is_empty());
    assert_eq!(
        key(&mut e, KeyCode::Char('d'), KeyModifiers::CONTROL),
        Some(Input::Exit)
    );
}
#[test]
fn narrow_prompt_keeps_a_wide_character_cursor_inside_the_terminal() {
    let mut e = Editor::default();
    type_text(&mut e, "long 你好 prompt\nmore λ words");
    for width in [1, 2, 4, 8, 20, 80] {
        let (text, col) = e.view("❯", width);
        assert!(col < width, "{col} {width}");
        assert!(text.chars().map(|c| c.width().unwrap_or(0)).sum::<usize>() < width);
        assert!(!text.contains('\n'));
    }
}
#[test]
fn terminal_controls_from_untrusted_output_are_inert() {
    assert_eq!(
        safe("ok\u{1b}]52;c;secret\u{7}\u{9b}2J\r\n\t"),
        "ok]52;c;secret2J\n\t"
    );
}

#[test]
fn word_line_and_delete_controls_keep_the_cursor_valid() {
    let mut e = Editor::default();
    type_text(&mut e, "one two ");
    key(&mut e, KeyCode::Char('w'), KeyModifiers::CONTROL);
    assert_eq!(e.text.iter().collect::<String>(), "one ");
    key(&mut e, KeyCode::Home, KeyModifiers::NONE);
    key(&mut e, KeyCode::Delete, KeyModifiers::NONE);
    key(&mut e, KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert_eq!(e.text.iter().collect::<String>(), "e ");
    key(&mut e, KeyCode::End, KeyModifiers::NONE);
    key(&mut e, KeyCode::Left, KeyModifiers::NONE);
    key(&mut e, KeyCode::Char('k'), KeyModifiers::CONTROL);
    assert_eq!(e.text.iter().collect::<String>(), "e");
    key(&mut e, KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert!(e.text.is_empty());
    assert_eq!(e.cursor, 0);
}
