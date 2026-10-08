//! Proves the persisted defaults: their values, validation, normalization and fallbacks.
use crate::support::{Server, replay::Replay};
use serde_json::{Value, json};
use std::fs;

fn put(server: &Server, body: Value) -> (u16, Value) {
    let response = server.call("PUT", "/api/settings/defaults", Some(body));
    (response.status, response.json())
}

#[test]
fn defaults_validate_normalize_and_ignore_unknown_keys() {
    let mut server = Server::new("defaults");
    let mut replay = Replay::new(&server);
    replay.case(&server, "defaults");
    replay.case(&server, "defaults-error");
    let error = |body: Value| {
        let (status, value) = put(&server, body);
        assert_eq!(status, 400, "{value}");
        value["error"].as_str().unwrap().to_owned()
    };
    for (key, low, high) in [
        ("reserve", 0, 50),
        ("quota_poll", 2, 240),
        ("approval_timeout", 1, 240),
        ("max_rounds", 1, 500),
        ("auto_continue", 0, 10),
    ] {
        for ok in [low, high] {
            assert_eq!(put(&server, json!({ key: ok })).1[key], ok.to_string());
        }
        for bad in [
            json!(low - 1),
            json!(high + 1),
            json!(f64::from(high) + 0.5),
        ] {
            assert_eq!(
                error(json!({ key: bad })),
                format!("{key}: between {low} and {high}")
            );
        }
        assert_eq!(error(json!({ key: "x" })), format!("{key}: a number"));
        assert_eq!(error(json!({ key: true })), format!("{key}: a number"));
    }
    // Numbers are stored as their shortest text, as the legacy app wrote them.
    assert_eq!(put(&server, json!({"reserve": "4.0"})).1["reserve"], "4");
    assert_eq!(put(&server, json!({"reserve": 2.5})).1["reserve"], "2.5");
    assert_eq!(
        put(&server, json!({"quota_poll": " 7 "})).1["quota_poll"],
        "7"
    );
    for rotation in ["sticky", "round-robin", "perishable"] {
        assert_eq!(
            put(&server, json!({ "rotation": rotation })).1["rotation"],
            rotation
        );
    }
    assert_eq!(
        error(json!({"rotation": "random"})),
        "rotation: perishable · sticky · round-robin"
    );
    for key in ["keep_awake", "worktree_default"] {
        assert_eq!(put(&server, json!({ key: "on" })).1[key], "on");
        assert_eq!(put(&server, json!({ key: "off" })).1[key], "off");
        assert_eq!(error(json!({ key: "yes" })), format!("{key}: on or off"));
        assert_eq!(error(json!({ key: true })), format!("{key}: on or off"));
    }
    let (_, saved) = put(
        &server,
        json!({"model": "gemini-pro-high", "bogus": 1, "verifier_model": "gemini-pro-low"}),
    );
    assert_eq!(
        (saved["model"].clone(), saved["verifier_model"].clone()),
        (json!("gemini-pro-high"), json!("gemini-pro-low"))
    );
    assert!(saved.get("bogus").is_none(), "unknown keys are ignored");
    // A refused update changes nothing.
    error(json!({"model": "gemini-3.8-flash-high", "reserve": 99}));
    let overview = server.call("GET", "/api/settings", None).json();
    assert_eq!(
        (
            overview["defaults"]["model"].clone(),
            overview["defaults"]["reserve"].clone()
        ),
        (json!("gemini-pro-high"), json!("2.5"))
    );
    let file: Value =
        serde_json::from_str(&fs::read_to_string(server.data.join("settings.json")).unwrap())
            .unwrap();
    assert!(file.get("bogus").is_none());
    server.stop();
}

#[test]
fn the_default_folder_expands_home_and_falls_back_to_it() {
    let mut server = Server::new("defaults-folder");
    let home = server.home.to_string_lossy().into_owned();
    assert_eq!(
        server.call("GET", "/api/state", None).json()["defaults"]["cwd"],
        home
    );
    let (status, saved) = put(&server, json!({"default_cwd": "~/project"}));
    assert_eq!(
        (status, saved["default_cwd"].clone()),
        (200, json!("~/project"))
    );
    let state = server.call("GET", "/api/state", None).json();
    assert_eq!(
        state["defaults"]["cwd"],
        server.project.to_string_lossy().as_ref()
    );
    let (status, refused) = put(&server, json!({"default_cwd": "~/missing"}));
    assert_eq!(
        (status, refused["error"].clone()),
        (400, json!("not a folder: ~/missing"))
    );
    fs::write(server.home.join("file.txt"), "x").unwrap();
    assert_eq!(put(&server, json!({"default_cwd": "~/file.txt"})).0, 400);
    assert_eq!(put(&server, json!({"default_cwd": ""})).0, 200);
    assert_eq!(
        server.call("GET", "/api/state", None).json()["defaults"]["cwd"],
        home
    );
    server.stop();
}

#[test]
fn hand_edited_values_survive_and_bad_numbers_refuse_further_saves() {
    let mut server = Server::new_stopped("defaults-file", false);
    fs::write(
        server.data.join("settings.json"),
        r#"{"keep_awake":"off","model":"gemini-pro-high","reserve":"abc","max_rounds":9}"#,
    )
    .unwrap();
    server.start();
    let defaults = server.call("GET", "/api/settings", None).json()["defaults"].clone();
    assert_eq!(
        (
            defaults["model"].clone(),
            defaults["reserve"].clone(),
            defaults["max_rounds"].clone()
        ),
        (json!("gemini-pro-high"), json!("abc"), json!("9"))
    );
    // The invalid stored number is reported by name until it is replaced.
    assert_eq!(
        put(&server, json!({"model": "gemini-3.8-flash-high"})).1["error"],
        "reserve: a number"
    );
    assert_eq!(put(&server, json!({"reserve": 3})).1["reserve"], "3");
    let id = server.create();
    for mode in ["accept-edits", "plan", "bypass", "default"] {
        let patched = server.call(
            "PATCH",
            &format!("/api/sessions/{id}"),
            Some(json!({ "mode": mode })),
        );
        assert_eq!(patched.json()["mode"], mode);
    }
    let refused = server.call(
        "PATCH",
        &format!("/api/sessions/{id}"),
        Some(json!({"mode": "fast"})),
    );
    assert_eq!(
        (refused.status, refused.json()["error"].clone()),
        (400, json!("unknown mode: fast"))
    );
    let created = server.call(
        "POST",
        "/api/sessions",
        Some(json!({"cwd": server.project, "mode": "turbo"})),
    );
    assert_eq!(
        (created.status, created.json()["error"].clone()),
        (400, json!("unknown mode: turbo"))
    );
    server.stop();
}
