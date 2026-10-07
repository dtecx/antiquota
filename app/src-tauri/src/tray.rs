//! The menu bar / system tray: what is running, how much quota each account has left, and
//! quick actions. It reads the engine's /api/state every few seconds.

use crate::{engine::Engine, show_main, Shared};
use chrono::{Local, TimeZone};
use serde_json::Value;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

pub const TRAY_ID: &str = "main";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray@2x.png"))?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .tooltip("antiquota")
        .menu(&build_menu(app, None)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| on_menu(app, event.id.as_ref()))
        .build(app)?;
    Ok(())
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => show_main(app),
        "new" => {
            show_main(app);
            eval(app, "typeof openNew === 'function' && openNew()");
        }
        "stop_all" => {
            let app = app.clone();
            std::thread::spawn(move || {
                if let Some(e) = engine(&app) {
                    if let Some(state) = e.get("api/state") {
                        for s in state["sessions"].as_array().into_iter().flatten() {
                            if s["status"] != "idle" {
                                e.post(&format!("api/sessions/{}/stop", s["id"].as_str().unwrap_or("")));
                            }
                        }
                    }
                }
            });
        }
        "quit" => crate::request_quit(app.clone()),
        other => {
            if let Some(sid) = other.strip_prefix("session:") {
                show_main(app);
                eval(app, &format!("typeof openSession === 'function' && openSession({sid:?})"));
            }
        }
    }
}

fn eval(app: &AppHandle, js: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(js);
    }
}

fn engine(app: &AppHandle) -> Option<Engine> {
    app.state::<Shared>().engine.lock().ok()?.clone()
}

fn short(email: &str) -> &str {
    email.split('@').next().unwrap_or(email)
}

fn hhmm(epoch: f64) -> String {
    Local
        .timestamp_opt(epoch as i64, 0)
        .single()
        .map(|t| t.format("%H:%M").to_string())
        .unwrap_or_default()
}

/// One account's line: Gemini quota left, or why it rests and until when.
fn account_line(acct: &Value) -> String {
    let name = short(acct["slot"].as_str().unwrap_or("?"));
    if acct["verify"].is_object() || acct["verify"].as_bool() == Some(true) {
        return format!("{name}  ·  needs verification");
    }
    let wall = acct["walls"]["gemini"].as_f64().unwrap_or(0.0);
    if wall > 0.0 {
        let why = acct["why"]["gemini"].as_str().unwrap_or("");
        let what = if why.starts_with("rate limited") || why.contains("errors") {
            "resting"
        } else if why.starts_with("signed out") {
            "signed out"
        } else {
            "out of Gemini quota"
        };
        return format!("{name}  ·  {what} until {}", hhmm(wall));
    }
    let left = acct["quota"]["gemini"]
        .as_object()
        .map(|w| w.values().filter_map(|x| x[0].as_f64()).fold(f64::INFINITY, f64::min))
        .filter(|v| v.is_finite());
    match left {
        Some(f) => format!("{name}  ·  Gemini {}%", (f * 100.0).round()),
        None => format!("{name}  ·  not probed yet"),
    }
}

pub fn build_menu(app: &AppHandle, state: Option<&Value>) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    let sessions: Vec<&Value> = state
        .and_then(|s| s["sessions"].as_array())
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let running: Vec<&Value> = sessions.iter().copied().filter(|s| s["status"] != "idle").collect();
    let jobs: usize = sessions
        .iter()
        .map(|s| s["jobs"].as_array().map(|j| j.iter().filter(|x| x["ended"].is_null()).count()).unwrap_or(0))
        .sum();
    let head = match (state.is_some(), running.len(), jobs) {
        (false, _, _) => "antiquota · starting…".to_string(),
        (true, 0, 0) => "antiquota · idle".to_string(),
        (true, r, j) => {
            let mut parts = vec![];
            if r > 0 {
                parts.push(format!("{r} working"));
            }
            if j > 0 {
                parts.push(format!("{j} job{}", if j > 1 { "s" } else { "" }));
            }
            format!("antiquota · {}", parts.join(", "))
        }
    };
    menu.append(&MenuItem::with_id(app, "head", head, false, None::<&str>)?)?;
    for s in running.iter().take(6) {
        let title = s["title"].as_str().filter(|t| !t.is_empty()).unwrap_or("Untitled");
        let status = s["status"].as_str().unwrap_or("");
        let label = format!("▶  {}  ·  {status}", title.chars().take(40).collect::<String>());
        let id = format!("session:{}", s["id"].as_str().unwrap_or(""));
        menu.append(&MenuItem::with_id(app, id, label, true, None::<&str>)?)?;
    }
    if let Some(accts) = state.and_then(|s| s["accounts"].as_array()).filter(|a| !a.is_empty()) {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        for (i, acct) in accts.iter().take(10).enumerate() {
            menu.append(&MenuItem::with_id(app, format!("acct{i}"), account_line(acct), false, None::<&str>)?)?;
        }
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "open", "Open antiquota", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "new", "New session…", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "stop_all", "Stop all running", !running.is_empty(), None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quit antiquota", true, Some("CmdOrCtrl+Q"))?)?;
    Ok(menu)
}

/// Keep the tray current, notice an engine that went away, and bring it back.
pub fn poll(app: AppHandle) {
    let mut misses = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let Some(e) = engine(&app) else { continue };
        let state = e.get("api/state");
        if state.is_none() {
            misses += 1;
            if misses >= 3 {
                misses = 0;
                crate::boot(&app); // engine gone (stopped elsewhere, crashed): start or find it again
            }
            continue;
        }
        misses = 0;
        let state = state.unwrap();
        let busy = state["sessions"]
            .as_array()
            .map(|a| a.iter().filter(|s| s["status"] != "idle").count())
            .unwrap_or(0);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(tray) = handle.tray_by_id(TRAY_ID) {
                if let Ok(menu) = build_menu(&handle, Some(&state)) {
                    let _ = tray.set_menu(Some(menu));
                }
                #[cfg(target_os = "macos")]
                let _ = tray.set_title(if busy > 0 { Some(busy.to_string()) } else { None });
                let _ = tray.set_tooltip(Some(if busy > 0 { format!("antiquota · {busy} working") } else { "antiquota".into() }));
            }
            if let Some(w) = handle.get_webview_window("main") {
                let _ = w.set_badge_count(if busy > 0 { Some(busy as i64) } else { None });
            }
        });
    }
}
