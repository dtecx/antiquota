//! The application menu dispatches typed actions through the native event bridge.

use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Wry};

fn item(
    app: &AppHandle,
    id: &str,
    label: &str,
    accel: Option<&str>,
) -> tauri::Result<MenuItem<Wry>> {
    MenuItem::with_id(app, id, label, true, accel)
}

pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let about = AboutMetadata {
        name: Some("Antiquota".into()),
        ..Default::default()
    };
    let app_menu = Submenu::with_items(
        app,
        "Antiquota",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("About Antiquota"), Some(about))?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, "settings", "Settings…", Some("CmdOrCtrl+Comma"))?,
            &item(app, "usage", "Accounts and Usage", None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, "quit", "Quit Antiquota", Some("CmdOrCtrl+Q"))?,
        ],
    )?;
    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &item(app, "new", "New Session", Some("CmdOrCtrl+N"))?,
            &item(
                app,
                "export",
                "Export Transcript…",
                Some("CmdOrCtrl+Shift+E"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, "find", "Find Session or Message", Some("CmdOrCtrl+K"))?,
            &item(app, "dictate", "Dictate", Some("CmdOrCtrl+Shift+M"))?,
        ],
    )?;
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &item(app, "zoom_in", "Zoom In", Some("CmdOrCtrl+Equal"))?,
            &item(app, "zoom_out", "Zoom Out", Some("CmdOrCtrl+Minus"))?,
            &item(app, "zoom_reset", "Actual Size", Some("CmdOrCtrl+0"))?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, "panel", "Side Panel", Some("CmdOrCtrl+B"))?,
            &item(app, "split", "Side by Side", Some("CmdOrCtrl+Backslash"))?,
            &item(app, "expand", "Expand All Steps", Some("CmdOrCtrl+Shift+O"))?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, "reload", "Reload", Some("CmdOrCtrl+R"))?,
            &PredefinedMenuItem::fullscreen(app, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &item(
                app,
                "prev_session",
                "Previous Session",
                Some("CmdOrCtrl+Shift+BracketLeft"),
            )?,
            &item(
                app,
                "next_session",
                "Next Session",
                Some("CmdOrCtrl+Shift+BracketRight"),
            )?,
        ],
    )?;
    let help = Submenu::with_items(
        app,
        "Help",
        true,
        &[
            &item(
                app,
                "shortcuts",
                "Keyboard Shortcuts",
                Some("CmdOrCtrl+Slash"),
            )?,
            &item(app, "engine_log", "Engine Log", None)?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &window, &help])
}

fn owns_action(id: &str) -> bool {
    matches!(
        id,
        "settings"
            | "usage"
            | "new"
            | "export"
            | "find"
            | "dictate"
            | "zoom_in"
            | "zoom_out"
            | "zoom_reset"
            | "panel"
            | "split"
            | "expand"
            | "reload"
            | "prev_session"
            | "next_session"
            | "shortcuts"
            | "engine_log"
            | "quit"
    )
}

pub fn on_event(app: &AppHandle, id: &str) {
    if !owns_action(id) {
        return;
    }
    if id == "quit" {
        crate::request_quit(app.clone());
        return;
    }
    crate::show_main(app);
    crate::native_action(app, id);
}

#[cfg(test)]
mod tests {
    use super::owns_action;

    #[test]
    fn application_listener_accepts_only_its_own_actions() {
        for id in [
            "settings",
            "usage",
            "new",
            "export",
            "find",
            "dictate",
            "zoom_in",
            "zoom_out",
            "zoom_reset",
            "panel",
            "split",
            "expand",
            "reload",
            "prev_session",
            "next_session",
            "shortcuts",
            "engine_log",
            "quit",
        ] {
            assert!(owns_action(id), "{id}");
            assert!(!owns_action(&format!("tray:{id}")), "{id}");
        }
        for id in [
            "open",
            "stop_all",
            "session:s0",
            "tray:session:s0",
            "unknown",
            "",
        ] {
            assert!(!owns_action(id), "{id}");
        }
    }
}
