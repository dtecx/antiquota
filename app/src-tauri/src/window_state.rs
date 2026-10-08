//! Restore the final window geometry while keeping hidden startup hidden.
use tauri::WebviewWindow;
use tauri_plugin_window_state::{StateFlags, WindowExt};

pub const FLAGS: StateFlags = StateFlags::SIZE
    .union(StateFlags::POSITION)
    .union(StateFlags::MAXIMIZED);

#[derive(Clone, Copy)]
struct Rect {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

fn title_visible(window: Rect, monitors: &[Rect]) -> bool {
    monitors.iter().any(|monitor| {
        let width =
            (window.x + window.width).min(monitor.x + monitor.width) - window.x.max(monitor.x);
        width >= 64 && window.y >= monitor.y && window.y + 32 <= monitor.y + monitor.height
    })
}

pub fn restore(window: &WebviewWindow, hidden: bool) -> tauri::Result<()> {
    window.restore_state(FLAGS)?;
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    let rectangle = Rect {
        x: position.x.into(),
        y: position.y.into(),
        width: size.width.into(),
        height: size.height.into(),
    };
    let monitors: Vec<_> = window
        .available_monitors()?
        .into_iter()
        .map(|monitor| Rect {
            x: monitor.position().x.into(),
            y: monitor.position().y.into(),
            width: monitor.size().width.into(),
            height: monitor.size().height.into(),
        })
        .collect();
    if !monitors.is_empty() && !title_visible(rectangle, &monitors) {
        window.center()?;
    }
    if hidden {
        window.hide()?;
    } else {
        window.show()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_size_and_title_visibility_determine_offscreen_fallback() {
        let monitor = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let window = Rect {
            x: 100,
            y: 100,
            width: 1280,
            height: 820,
        };
        assert!(title_visible(window, &[monitor]));
        assert!(!title_visible(Rect { x: 2200, ..window }, &[monitor]));
        assert!(!title_visible(Rect { x: 1900, ..window }, &[monitor]));
        assert!(!title_visible(Rect { y: -40, ..window }, &[monitor]));
        assert!(!title_visible(Rect { y: 1060, ..window }, &[monitor]));
        assert!(title_visible(Rect { x: -1200, ..window }, &[monitor]));
        assert!(!title_visible(
            Rect {
                x: -1200,
                width: 1000,
                ..window
            },
            &[monitor]
        ));
        let left = Rect {
            x: -1920,
            ..monitor
        };
        assert!(title_visible(Rect { x: -1500, ..window }, &[left, monitor]));
        assert!(!title_visible(Rect { x: -1500, ..window }, &[monitor]));
    }
}
