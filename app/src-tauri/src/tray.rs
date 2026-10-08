//! Tray status and quick actions, refreshed from the engine every five seconds.

use crate::{Shared, engine::Engine, show_main};
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
    let Some(id) = action_payload(id) else { return };
    match id {
        "open" => show_main(app),
        "new" => {
            show_main(app);
            crate::native_action(app, "new");
        }
        "stop_all" => {
            let app = app.clone();
            std::thread::spawn(move || {
                if let Some(e) = engine(&app)
                    && let Some(state) = e.get("api/state")
                {
                    for path in stop_paths(&state) {
                        e.post(&path);
                    }
                }
            });
        }
        "quit" => crate::request_quit(app.clone()),
        other => {
            show_main(app);
            crate::native_action(app, other);
        }
    }
}

fn action_payload(id: &str) -> Option<&str> {
    let action = id.strip_prefix("tray:")?;
    (matches!(action, "open" | "new" | "stop_all" | "quit")
        || action
            .strip_prefix("session:")
            .is_some_and(|sid| !sid.is_empty()))
    .then_some(action)
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
fn account_line(acct: &Value, now: f64) -> String {
    let name = short(acct["slot"].as_str().unwrap_or("?"));
    if acct["verify"]
        .as_str()
        .is_some_and(|url| !url.trim().is_empty())
        || acct["verify"].is_object()
        || acct["verify"].as_bool() == Some(true)
    {
        return format!("{name}  ·  needs verification");
    }
    let wall = acct["walls"]["gemini"].as_f64().unwrap_or(0.0);
    if wall > now {
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
        .map(|w| {
            w.values()
                .filter_map(|x| x[0].as_f64())
                .fold(f64::INFINITY, f64::min)
        })
        .filter(|v| v.is_finite());
    match left {
        Some(f) => format!("{name}  ·  Gemini {}%", (f * 100.0).round()),
        None => format!("{name}  ·  not probed yet"),
    }
}

fn sessions(state: Option<&Value>) -> &[Value] {
    state
        .and_then(|s| s["sessions"].as_array())
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn activity(state: Option<&Value>) -> (usize, usize) {
    let sessions = sessions(state);
    let running = sessions.iter().filter(|s| s["status"] != "idle").count();
    let jobs: usize = sessions
        .iter()
        .map(|s| {
            s["jobs"]
                .as_array()
                .map(|j| j.iter().filter(|x| x["ended"].is_null()).count())
                .unwrap_or(0)
        })
        .sum();
    (running, jobs)
}

fn heading(state: Option<&Value>) -> String {
    let (running, jobs) = activity(state);
    match (state.is_some(), running, jobs) {
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
    }
}

fn stop_paths(state: &Value) -> Vec<String> {
    sessions(Some(state))
        .iter()
        .filter(|s| s["status"] != "idle")
        .filter_map(|s| s["id"].as_str().filter(|id| !id.is_empty()))
        .map(|id| format!("api/sessions/{id}/stop"))
        .collect()
}

fn session_entries(state: Option<&Value>) -> Vec<(String, String)> {
    sessions(state)
        .iter()
        .filter(|s| s["status"] != "idle")
        .take(6)
        .map(|s| {
            let title = s["title"]
                .as_str()
                .filter(|t| !t.is_empty())
                .unwrap_or("Untitled");
            let title = title.chars().take(40).collect::<String>();
            let status = s["status"].as_str().unwrap_or("");
            (
                format!("tray:session:{}", s["id"].as_str().unwrap_or("")),
                format!("▶  {title}  ·  {status}"),
            )
        })
        .collect()
}

fn account_entries(state: Option<&Value>, now: f64) -> Vec<String> {
    state
        .and_then(|s| s["accounts"].as_array())
        .into_iter()
        .flatten()
        .take(10)
        .map(|acct| account_line(acct, now))
        .collect()
}

fn should_respawn(misses: &mut u8, available: bool) -> bool {
    if available {
        *misses = 0;
        return false;
    }
    *misses += 1;
    if *misses < 3 {
        return false;
    }
    *misses = 0;
    true
}

pub fn build_menu(app: &AppHandle, state: Option<&Value>) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    let item =
        |id: &str, label: &str, enabled| MenuItem::with_id(app, id, label, enabled, None::<&str>);
    let head = heading(state);
    menu.append(&item("tray:head", &head, false)?)?;
    for (id, label) in session_entries(state) {
        menu.append(&item(&id, &label, true)?)?;
    }
    let accounts = account_entries(state, chrono::Utc::now().timestamp() as f64);
    if !accounts.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        for (i, label) in accounts.iter().enumerate() {
            menu.append(&item(&format!("tray:acct{i}"), label, false)?)?;
        }
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&item("tray:open", "Open antiquota", true)?)?;
    menu.append(&item("tray:new", "New session…", true)?)?;
    menu.append(&item(
        "tray:stop_all",
        "Stop all running",
        activity(state).0 > 0,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "tray:quit",
        "Quit antiquota",
        true,
        Some("CmdOrCtrl+Q"),
    )?)?;
    Ok(menu)
}

/// Keep the tray current, notice an engine that went away, and bring it back.
pub fn poll(app: AppHandle) {
    let mut misses = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let Some(e) = engine(&app) else { continue };
        let state = e.get("api/state");
        if should_respawn(&mut misses, state.is_some()) {
            crate::boot(&app); // Rediscover or restart after three failed state requests.
        }
        let Some(state) = state else { continue };
        let busy = activity(Some(&state)).0;
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(tray) = handle.tray_by_id(TRAY_ID) {
                if let Ok(menu) = build_menu(&handle, Some(&state)) {
                    let _ = tray.set_menu(Some(menu));
                }
                #[cfg(target_os = "macos")]
                let _ = tray.set_title(if busy > 0 {
                    Some(busy.to_string())
                } else {
                    None
                });
                let _ = tray.set_tooltip(Some(if busy > 0 {
                    format!("antiquota · {busy} working")
                } else {
                    "antiquota".into()
                }));
            }
            if let Some(w) = handle.get_webview_window("main") {
                let _ = w.set_badge_count(if busy > 0 { Some(busy as i64) } else { None });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn verification_accepts_api_urls_and_legacy_shapes_but_not_null_or_empty() {
        for verify in [
            json!("https://accounts.google.com/verify"),
            json!({}),
            json!(true),
        ] {
            assert_eq!(
                account_line(&json!({"slot":"one@example.com","verify":verify}), 100.0),
                "one  ·  needs verification"
            );
        }
        for verify in [Value::Null, json!(""), json!("  "), json!(false)] {
            assert_eq!(
                account_line(&json!({"slot":"one@example.com","verify":verify}), 100.0),
                "one  ·  not probed yet"
            );
        }
    }

    #[test]
    fn quota_uses_lowest_gemini_window_and_only_future_walls_override_it() {
        let mut account = json!({"slot":"one@example.com","verify":null,
            "quota":{"gemini":{"5h":[0.855,null],"weekly":[0.234,2000]},"cg":{"weekly":[0.01,null]}},
            "walls":{"gemini":100},"why":{"gemini":"quota"}});
        assert_eq!(account_line(&account, 101.0), "one  ·  Gemini 23%");
        assert_eq!(account_line(&account, 100.0), "one  ·  Gemini 23%");
        account["walls"]["gemini"] = json!(2000);
        for (why, label) in [
            ("quota", "out of Gemini quota"),
            ("rate limited twice", "resting"),
            ("temporary errors", "resting"),
            ("signed out", "signed out"),
        ] {
            account["why"]["gemini"] = json!(why);
            assert!(account_line(&account, 100.0).starts_with(&format!("one  ·  {label} until ")));
        }
        account["walls"]["gemini"] = json!(0);
        account["quota"]["gemini"]["5h"] = json!([0, null]);
        assert_eq!(account_line(&account, 100.0), "one  ·  Gemini 0%");
    }

    #[test]
    fn heading_distinguishes_startup_idle_and_unfinished_background_jobs() {
        assert_eq!(heading(None), "antiquota · starting…");
        assert_eq!(heading(Some(&json!({"sessions":[]}))), "antiquota · idle");
        let state = json!({"sessions":[{"status":"running","jobs":[{"ended":null},{"ended":1}]},
                                       {"status":"waiting","jobs":[{"ended":null}]},
                                       {"status":"idle","jobs":[{"ended":null}]}]});
        assert_eq!(activity(Some(&state)), (2, 3));
        assert_eq!(heading(Some(&state)), "antiquota · 2 working, 3 jobs");
        let jobs_only = json!({"sessions":[{"status":"idle","jobs":[{"ended":null}]}]});
        assert_eq!(heading(Some(&jobs_only)), "antiquota · 1 job");
    }

    #[test]
    fn menu_entries_are_capped_but_stop_all_keeps_every_working_session() {
        let mut rows: Vec<Value> = (0..8)
            .map(|i| {
                json!({"id":format!("s{i}"),
            "title":"界".repeat(45),"status":"waiting"})
            })
            .collect();
        rows.push(json!({"id":"idle","status":"idle"}));
        let state = json!({"sessions":rows,"accounts":(0..12).map(|i|
            json!({"slot":format!("a{i}@example.com")})).collect::<Vec<_>>()});
        let entries = session_entries(Some(&state));
        assert_eq!(entries.len(), 6);
        assert_eq!(
            entries[0],
            (
                "tray:session:s0".into(),
                format!("▶  {}  ·  waiting", "界".repeat(40))
            )
        );
        assert_eq!(entries[5].0, "tray:session:s5");
        assert_eq!(account_entries(Some(&state), 100.0).len(), 10);
        assert_eq!(
            stop_paths(&state),
            (0..8)
                .map(|i| format!("api/sessions/s{i}/stop"))
                .collect::<Vec<_>>()
        );
        assert_eq!(activity(Some(&state)).0, 8);
        assert!(stop_paths(&json!({"sessions":[{"id":"idle","status":"idle"}]})).is_empty());
    }

    #[test]
    fn respawn_requires_three_consecutive_state_failures() {
        let mut misses = 0;
        assert!(!should_respawn(&mut misses, false));
        assert!(!should_respawn(&mut misses, false));
        assert!(!should_respawn(&mut misses, true));
        assert!(!should_respawn(&mut misses, false));
        assert!(!should_respawn(&mut misses, false));
        assert!(should_respawn(&mut misses, false));
        assert_eq!(misses, 0);
        assert!(!should_respawn(&mut misses, false));
    }

    #[test]
    fn tray_listener_accepts_only_its_namespace_and_keeps_bridge_payloads() {
        for action in ["open", "new", "stop_all", "quit", "session:s0"] {
            assert_eq!(action_payload(&format!("tray:{action}")), Some(action));
            assert_eq!(action_payload(action), None);
        }
        for id in [
            "tray:session:",
            "tray:unknown",
            "tray:acct0",
            "tray:settings",
            "",
        ] {
            assert_eq!(action_payload(id), None);
        }
    }
}
