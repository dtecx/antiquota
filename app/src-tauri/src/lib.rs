//! antiquota desktop app: a native window, tray and notifications around the antiquota engine.
//!
//! The window first shows a bundled start page, then loads the engine's own UI from
//! http://127.0.0.1:<port>/ once the engine answers. The engine keeps running when the window
//! closes (the app stays in the tray), and quitting asks before stopping running work.

mod engine;
mod tray;

use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

pub struct Shared {
    pub engine: Mutex<Option<engine::Engine>>,
}

static QUITTING: AtomicBool = AtomicBool::new(false);
static BOOTING: AtomicBool = AtomicBool::new(false);

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn status(app: &AppHandle, text: &str, error: bool) {
    let _ = app.emit("engine-status", json!({ "text": text, "error": error }));
}

/// Find or start the engine, then point the window at it. Safe to call again (retry, restart).
pub fn boot(app: &AppHandle) {
    if BOOTING.swap(true, Ordering::SeqCst) {
        return;
    }
    status(app, "Starting the antiquota engine…", false);
    let resources = app.path().resource_dir().ok();
    match engine::ensure(resources.as_deref()) {
        Ok(e) => {
            let url = e.url.clone();
            *app.state::<Shared>().engine.lock().unwrap() = Some(e);
            if let (Some(w), Ok(u)) = (app.get_webview_window("main"), url.parse()) {
                let _ = w.navigate(u);
            }
        }
        Err(msg) => status(app, &msg, true),
    }
    BOOTING.store(false, Ordering::SeqCst);
}

#[tauri::command]
fn app_info(app: AppHandle) -> serde_json::Value {
    json!({ "version": app.package_info().version.to_string(), "os": std::env::consts::OS })
}

#[tauri::command]
fn retry_engine(app: AppHandle) {
    std::thread::spawn(move || boot(&app));
}

/// Save text where the user picks (the web view cannot download files).
#[tauri::command]
async fn save_file(app: AppHandle, name: String, contents: String) -> Result<Option<String>, String> {
    let Some(path) = app.dialog().file().set_file_name(&name).blocking_save_file() else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, contents).map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    request_quit(app);
}

/// Quit, asking first when agents or jobs are running in an engine this app started.
pub fn request_quit(app: AppHandle) {
    std::thread::spawn(move || {
        let e = app.state::<Shared>().engine.lock().ok().and_then(|g| g.clone());
        if let Some(e) = e.filter(|e| e.by_app) {
            let state = e.get("api/state");
            let working = state
                .as_ref()
                .and_then(|s| s["sessions"].as_array())
                .map(|a| a.iter().filter(|s| s["status"] != "idle").count())
                .unwrap_or(0);
            let jobs = state
                .as_ref()
                .and_then(|s| s["sessions"].as_array())
                .map(|a| {
                    a.iter()
                        .map(|s| s["jobs"].as_array().map(|j| j.iter().filter(|x| x["ended"].is_null()).count()).unwrap_or(0))
                        .sum::<usize>()
                })
                .unwrap_or(0);
            let stop = if working + jobs == 0 {
                true
            } else {
                let mut what = vec![];
                if working > 0 {
                    what.push(format!("{working} session{} working", if working > 1 { "s are" } else { " is" }));
                }
                if jobs > 0 {
                    what.push(format!("{jobs} background job{} running", if jobs > 1 { "s are" } else { " is" }));
                }
                show_main(&app);
                app.dialog()
                    .message(format!(
                        "{}.\n\nKeep them running in the background and reopen antiquota later, or stop the \
                         sessions and quit. Background jobs keep running either way.",
                        what.join(" and ")
                    ))
                    .title("Quit antiquota?")
                    .kind(MessageDialogKind::Warning)
                    .buttons(MessageDialogButtons::OkCancelCustom(
                        "Keep running in background".into(),
                        "Stop and quit".into(),
                    ))
                    .blocking_show()
                    == false
            };
            if stop {
                e.post("api/shutdown");
            }
        }
        QUITTING.store(true, Ordering::SeqCst);
        app.exit(0);
    });
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]), // started at login: straight to the tray
        ))
        .manage(Shared { engine: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![app_info, retry_engine, save_file, quit_app])
        .setup(|app| {
            let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("antiquota")
                .inner_size(1280.0, 820.0)
                .min_inner_size(720.0, 480.0)
                .background_color(tauri::window::Color(14, 15, 17, 255))
                .disable_drag_drop_handler() // HTML5 drag and drop reaches the page (attachments)
                .visible(!std::env::args().any(|a| a == "--hidden"));
            // macOS: the sidebar runs under the traffic lights, like other native apps.
            #[cfg(target_os = "macos")]
            let builder = builder
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true)
                .traffic_light_position(tauri::LogicalPosition::new(18.0, 20.0));
            builder.build()?;
            tray::create(app.handle())?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                boot(&handle);
                tray::poll(handle);
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_close(); // the app lives on in the tray; Quit is in the tray menu
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building antiquota")
        .run(|app, event| match event {
            RunEvent::ExitRequested { api, .. } => {
                if !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_exit();
                    request_quit(app.clone());
                }
            }
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => show_main(app),
            _ => {}
        });
}
