//! antiquota desktop app: a native window, tray and notifications around the antiquota engine.
//!
//! The window first shows a bundled start page, then loads the engine's own UI from
//! http://127.0.0.1:<port>/ once the engine answers. The engine keeps running when the window
//! closes (the app stays in the tray), and quitting asks before stopping running work.

mod dialogs;
mod engine;
mod menu;
mod tray;
mod window_state;

use serde_json::json;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

pub struct Shared {
    pub engine: Mutex<Option<engine::Engine>>,
}

static QUITTING: AtomicBool = AtomicBool::new(false);
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
static BOOTING: AtomicBool = AtomicBool::new(false);

pub fn native_action(app: &AppHandle, action: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("native-action", action);
    }
}

fn background_proof() -> bool {
    cfg!(debug_assertions) && std::env::var_os("ANTIQUOTA_BACKGROUND_TEST").is_some()
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        if !background_proof() {
            let _ = w.set_focus();
        }
    }
}

fn status(app: &AppHandle, text: &str, error: bool) {
    let _ = app.emit("engine-status", json!({ "text": text, "error": error }));
}

struct BootGuard;
impl Drop for BootGuard {
    fn drop(&mut self) {
        BOOTING.store(false, Ordering::SeqCst);
    }
}

fn connect_engine(app: &AppHandle) -> Result<(), String> {
    status(app, "Starting the antiquota engine…", false);
    let resources = app.path().resource_dir().ok();

    let engine = engine::ensure(resources.as_deref())?;

    let url = engine
        .url
        .parse()
        .map_err(|error| format!("Invalid engine URL: {error}"))?;
    *app.state::<Shared>().engine.lock().unwrap() = Some(engine);

    if let Some(window) = app.get_webview_window("main")
        && needs_navigation(window.url().ok().as_ref(), &url)
    {
        window
            .navigate(url)
            .map_err(|error| format!("The engine page could not open: {error}"))?;
    }

    Ok(())
}

fn needs_navigation(current: Option<&tauri::Url>, engine: &tauri::Url) -> bool {
    !current.is_some_and(|url| url.origin() == engine.origin())
}

/// Find or start the engine, then point the window at it. Safe to call again (retry, restart).
pub fn boot(app: &AppHandle) {
    if BOOTING.swap(true, Ordering::SeqCst) {
        return;
    }
    let _guard = BootGuard;
    if let Err(error) = connect_engine(app) {
        status(app, &error, true);
    }
}

#[tauri::command]
fn app_info(app: AppHandle) -> serde_json::Value {
    json!({ "version": app.package_info().version.to_string(), "os": std::env::consts::OS })
}

#[tauri::command]
fn retry_engine(app: AppHandle) {
    std::thread::spawn(move || boot(&app));
}

#[tauri::command]
async fn restart_engine(app: AppHandle) -> Result<(), String> {
    if BOOTING.swap(true, Ordering::SeqCst) {
        return Err("The engine is already starting or restarting.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = BootGuard;
        let result = (|| {
            let engine = app
                .state::<Shared>()
                .engine
                .lock()
                .ok()
                .and_then(|value| value.clone());
            if let Some(engine) = engine {
                if !engine.post("api/shutdown") {
                    return Err("The engine refused to restart.".into());
                }
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                while engine::healthy(&engine.url) {
                    if std::time::Instant::now() >= deadline {
                        return Err("The engine did not stop. Try again.".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
            connect_engine(&app)
        })();
        if let Err(error) = &result {
            status(&app, error, true);
        }
        result
    })
    .await
    .map_err(|error| format!("Engine restart failed: {error}"))?
}

/// Save text where the user picks (the web view cannot download files).
#[tauri::command]
fn quit_app(app: AppHandle) {
    request_quit(app);
}

/// Quit, asking first when agents or jobs are running in an engine this app started.
pub fn request_quit(app: AppHandle) {
    if QUIT_REQUESTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let e = app
            .state::<Shared>()
            .engine
            .lock()
            .ok()
            .and_then(|g| g.clone());
        if let Some(e) = e.filter(|e| e.by_app) {
            let state = e.get("api/state");
            let work = active_work(state.as_ref());
            let stop = if work == Some((0, 0)) {
                true
            } else {
                let description = match work {
                    Some((sessions, jobs)) => {
                        format!("{sessions} session(s) and {jobs} background job(s) are working.")
                    }
                    None => "The engine state is unavailable. It may still be working.".into(),
                };
                show_main(&app);
                app.dialog().message(format!(
                    "{description}\n\nKeep work running in the background and reopen antiquota later, or stop sessions and quit. Background jobs keep running either way."
                )).title("Quit antiquota?").kind(MessageDialogKind::Warning)
                    .buttons(MessageDialogButtons::OkCancelCustom("Stop and quit".into(), "Keep running in background".into()))
                    .blocking_show()
            };
            if stop && let Err(error) = shutdown_for_quit(&e, &QUIT_REQUESTED) {
                show_main(&app);
                app.dialog()
                    .message(error)
                    .title("Could not stop antiquota")
                    .kind(MessageDialogKind::Error)
                    .buttons(MessageDialogButtons::Ok)
                    .blocking_show();
                return;
            }
        }
        QUITTING.store(true, Ordering::SeqCst);
        app.exit(0);
    });
}

fn shutdown_for_quit(engine: &engine::Engine, requested: &AtomicBool) -> Result<(), &'static str> {
    if engine.post("api/shutdown") {
        Ok(())
    } else {
        requested.store(false, Ordering::SeqCst);
        Err(
            "The engine did not accept the shutdown request. antiquota will stay open. Check Engine Log in the Help menu, then retry Quit.",
        )
    }
}

fn active_work(state: Option<&serde_json::Value>) -> Option<(usize, usize)> {
    let sessions = state?.get("sessions")?.as_array()?;
    let mut working = 0;
    let mut jobs = 0;
    for session in sessions {
        let status = session.get("status")?.as_str()?;
        if status.is_empty() {
            return None;
        }
        working += usize::from(status != "idle");
        for job in session.get("jobs")?.as_array()? {
            let ended = job.get("ended")?;
            if !ended.is_null() && !ended.is_number() {
                return None;
            }
            jobs += usize::from(ended.is_null());
        }
    }
    Some((working, jobs))
}

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.activate_ignoring_other_apps(
        !background_proof() && !std::env::args().any(|argument| argument == "--hidden"),
    );
    builder
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(window_state::FLAGS)
                .skip_initial_state("main")
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]), // started at login: straight to the tray
        ))
        .manage(Shared {
            engine: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            retry_engine,
            dialogs::save_file,
            dialogs::pick_folder,
            quit_app,
            restart_engine
        ])
        .setup(|app| {
            let builder =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("antiquota")
                    .inner_size(1280.0, 820.0)
                    .min_inner_size(720.0, 480.0)
                    .center() // the fallback when the saved monitor is no longer connected
                    .background_color(tauri::window::Color(14, 15, 17, 255))
                    .disable_drag_drop_handler() // HTML5 drag and drop reaches the page (attachments)
                    .focused(!background_proof())
                    .visible(false);
            // macOS: the sidebar runs under the traffic lights, like other native apps.
            #[cfg(target_os = "macos")]
            let builder = builder
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true)
                .traffic_light_position(tauri::LogicalPosition::new(18.0, 20.0));
            let navigation_app = app.handle().clone();
            let window = builder
                .on_navigation(move |url| {
                    if (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
                        || (matches!(url.scheme(), "http" | "https")
                            && url.host_str() == Some("tauri.localhost"))
                    {
                        return true;
                    }
                    navigation_app
                        .state::<Shared>()
                        .engine
                        .lock()
                        .ok()
                        .and_then(|value| {
                            value
                                .as_ref()
                                .and_then(|engine| engine.url.parse::<tauri::Url>().ok())
                        })
                        .is_some_and(|engine| engine.origin() == url.origin())
                })
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .build()?;
            window_state::restore(
                &window,
                background_proof() || std::env::args().any(|argument| argument == "--hidden"),
            )?;
            tray::create(app.handle())?;
            app.set_menu(menu::build(app.handle())?)?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                tray::poll(handle);
            });
            Ok(())
        })
        .on_menu_event(|app, event| menu::on_event(app, event.id().as_ref()))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
                && !QUITTING.load(Ordering::SeqCst)
            {
                api.prevent_close(); // the app lives on in the tray; Quit is in the tray menu
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building antiquota")
        .run(|app, event| match event {
            RunEvent::Ready
                if background_proof()
                    && !std::env::args().any(|argument| argument == "--hidden") =>
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }
            RunEvent::ExitRequested { api, .. } if !QUITTING.load(Ordering::SeqCst) => {
                api.prevent_exit();
                request_quit(app.clone());
            }
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => show_main(app),
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_opener_has_explicit_http_scopes_and_can_open_private_home_paths() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/engine-ui.json")).unwrap();
        assert_eq!(
            capability["remote"]["urls"],
            json!(["http://127.0.0.1:*/*", "http://localhost:*/*"])
        );
        let permission = capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|permission| permission["identifier"] == "opener:allow-open-url")
            .unwrap();
        assert_eq!(
            permission["allow"],
            json!([{"url":"http://*"},{"url":"https://*"}])
        );
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(
            config["plugins"]["opener"]["requireLiteralLeadingDot"],
            false
        );
    }

    #[test]
    fn refused_shutdown_keeps_quit_retryable_until_the_engine_accepts() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let engine = engine::Engine {
            url: format!("http://{}/", listener.local_addr().unwrap()),
            token: "test-token".into(),
            by_app: true,
        };
        let server = std::thread::spawn(move || {
            for status in ["503 Service Unavailable", "200 OK"] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut request = vec![];
                let mut bytes = [0; 1024];
                while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    let count = stream.read(&mut bytes).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&bytes[..count]);
                }
                let body = request
                    .windows(4)
                    .position(|bytes| bytes == b"\r\n\r\n")
                    .unwrap()
                    + 4;
                while request.len() < body + 2 {
                    let count = stream.read(&mut bytes).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&bytes[..count]);
                }
                assert_eq!(&request[body..body + 2], b"{}");
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with("POST /api/shutdown HTTP/1.1\r\n"));
                assert!(
                    request
                        .to_lowercase()
                        .contains("x-antiquota-token: test-token\r\n")
                );
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
                )
                .unwrap();
            }
        });
        let requested = AtomicBool::new(true);
        let error = shutdown_for_quit(&engine, &requested).unwrap_err();
        assert!(error.contains("stay open") && error.contains("retry Quit"));
        assert!(
            !requested.swap(true, Ordering::SeqCst),
            "a refused quit must admit a retry"
        );
        assert!(shutdown_for_quit(&engine, &requested).is_ok());
        assert!(
            requested.load(Ordering::SeqCst),
            "an accepted quit remains claimed until exit"
        );
        server.join().unwrap();
    }
    #[test]
    fn rediscovery_preserves_same_origin_drafts_and_selection() {
        let engine = "http://127.0.0.1:8765/".parse().unwrap();
        assert!(needs_navigation(None, &engine));
        for current in [
            "tauri://localhost/",
            "http://127.0.0.1:8766/",
            "https://example.invalid/",
        ] {
            assert!(needs_navigation(Some(&current.parse().unwrap()), &engine));
        }
        for current in [
            "http://127.0.0.1:8765/",
            "http://127.0.0.1:8765/#session-id",
        ] {
            assert!(!needs_navigation(Some(&current.parse().unwrap()), &engine));
        }
    }
    #[test]
    fn quit_never_interprets_unknown_work_as_idle() {
        assert_eq!(active_work(None), None);
        for value in [
            json!({}),
            json!({"sessions":null}),
            json!({"sessions":[{}]}),
            json!({"sessions":[{"status":"idle"}]}),
            json!({"sessions":[{"status":"idle","jobs":[{}]}]}),
        ] {
            assert_eq!(active_work(Some(&value)), None);
        }
        assert_eq!(active_work(Some(&json!({"sessions":[]}))), Some((0, 0)));
        assert_eq!(
            active_work(Some(&json!({"sessions":[{"status":"idle","jobs":[]}]}))),
            Some((0, 0))
        );
        assert_eq!(
            active_work(Some(
                &json!({"sessions":[{"status":"working","jobs":[{"ended":null},{"ended":123.0}]},{"status":"idle","jobs":[{"ended":null}]}]})
            )),
            Some((1, 2))
        );
    }
}
