//! The app menu (the macOS menu bar; the window menu on Windows and Linux). Most items call
//! the page's own functions, so the keys work the same in the app and in a browser.

use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Wry};

fn item(app: &AppHandle, id: &str, label: &str, accel: Option<&str>) -> tauri::Result<MenuItem<Wry>> {
    MenuItem::with_id(app, id, label, true, accel)
}

pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let about = AboutMetadata { name: Some("antiquota".into()), ..Default::default() };
    let app_menu = Submenu::with_items(app, "antiquota", true, &[
        &PredefinedMenuItem::about(app, Some("About antiquota"), Some(about))?,
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
        &item(app, "quit", "Quit antiquota", Some("CmdOrCtrl+Q"))?,
    ])?;
    let file = Submenu::with_items(app, "File", true, &[
        &item(app, "new", "New Session", Some("CmdOrCtrl+N"))?,
        &item(app, "export", "Export Transcript…", Some("CmdOrCtrl+Shift+E"))?,
        &PredefinedMenuItem::separator(app)?,
        &PredefinedMenuItem::close_window(app, None)?,
    ])?;
    let edit = Submenu::with_items(app, "Edit", true, &[
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
    ])?;
    let view = Submenu::with_items(app, "View", true, &[
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
    ])?;
    let window = Submenu::with_items(app, "Window", true, &[
        &PredefinedMenuItem::minimize(app, None)?,
        &PredefinedMenuItem::maximize(app, None)?,
        &PredefinedMenuItem::separator(app)?,
        &item(app, "prev_session", "Previous Session", Some("CmdOrCtrl+Shift+BracketLeft"))?,
        &item(app, "next_session", "Next Session", Some("CmdOrCtrl+Shift+BracketRight"))?,
    ])?;
    let help = Submenu::with_items(app, "Help", true, &[
        &item(app, "shortcuts", "Keyboard Shortcuts", Some("CmdOrCtrl+Slash"))?,
        &item(app, "engine_log", "Engine Log", None)?,
    ])?;
    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &window, &help])
}

pub fn on_event(app: &AppHandle, id: &str) {
    let js = match id {
        "settings" => "openSettings('general')",
        "usage" => "openSettings('usage')",
        "new" => "openNew()",
        "export" => "exportTranscript()",
        "find" => "($('#search').focus(), $('#search').select())",
        "dictate" => "toggleDictation()",
        "zoom_in" => "zoomBy(0.1)",
        "zoom_out" => "zoomBy(-0.1)",
        "zoom_reset" => "zoomBy(0)",
        "panel" => "$('#drawer-btn').click()",
        "split" => "($('#split').hidden ? $('#split-btn').click() : closeSplit())",
        "expand" => "$('#expand-btn').click()",
        "reload" => "location.reload()",
        "prev_session" | "next_session" => if id == "prev_session" { "stepSession(-1)" } else { "stepSession(1)" },
        "shortcuts" => "showShortcuts()",
        "engine_log" => "api('GET', 'engine/log').then(r => showFile({ rel: 'engine log', path: 'engine.log', text: r.text }))",
        "quit" => {
            crate::request_quit(app.clone());
            return;
        }
        _ => return, // tray items and predefined ones handle themselves
    };
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.eval(&format!("try {{ {js} }} catch (e) {{ console.error(e) }}"));
    }
}
