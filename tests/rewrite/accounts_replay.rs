//! Replays immutable account, login, routing, and defaults captures exactly.
use crate::{
    setup,
    support::{Server, replay::Replay},
};
use serde_json::json;
use std::fs;

#[test]
fn captured_account_routes_preserve_exact_overviews_and_routing_records() {
    let mut server = Server::new_stopped("accounts-replay", true);
    setup::captured_home(&server);
    let lease = antiquota_store::lock_file(&server.data.join("state/leases/one@example.com.lock"))
        .unwrap()
        .unwrap();
    server.start();
    let mut replay = Replay::new(&server);
    replay.case(&server, "accounts-initial");
    replay.case(&server, "accounts-probe");
    replay.case(&server, "account-check");
    setup::wait(&server, "second account probe ends", |accounts| {
        accounts[1]["busy"] == false
    });
    drop(lease);
    for name in [
        "account-check-idle",
        "account-pause",
        "account-resume",
        "account-missing-check",
        "routing",
        "account-delete-invalid",
    ] {
        replay.case(&server, name);
    }
    let third = server
        .data
        .join("slots/three@example.com/.gemini/antigravity-cli/antigravity-oauth-token");
    setup::write(&third, "synthetic-third-token");
    replay.case(&server, "account-delete");
    assert!(!third.exists());
    setup::wait(&server, "delete refresh settles", |rows| {
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| row["busy"] == false)
    });
    let lease = antiquota_store::lock_file(&server.data.join("state/leases/one@example.com.lock"))
        .unwrap()
        .unwrap();
    let busy_delete = server.call("DELETE", "/api/accounts/one@example.com", None);
    assert_eq!(busy_delete.status, 400);
    assert!(
        server
            .data
            .join("slots/one@example.com/.gemini/antigravity-cli/antigravity-oauth-token")
            .is_file()
    );
    drop(lease);
    setup::evidence(
        "accounts-contract",
        json!({"capturedCases":["accounts-initial","accounts-probe","account-check","account-check-idle","account-pause","account-resume","account-missing-check","routing","account-delete-invalid","account-delete"],"exactJson":true,"busyDeleteRejected":true}),
    );
    server.stop();
}
#[test]
fn captured_login_routes_adopt_cancel_and_replace_without_opening_browser() {
    let mut server = Server::new_stopped("login-replay", true);
    setup::captured_home(&server);
    setup::script(&server, json!({"login":{"email":"three@example.com"}}));
    setup::start(&mut server);
    let mut replay = Replay::new(&server);
    replay.case(&server, "login-start");
    replay.case(&server, "login-poll");
    replay.case(&server, "login-code-invalid");
    replay.case(&server, "login-submit");
    let auth = server
        .data
        .join("slots/three@example.com/.gemini/antigravity-cli/antigravity-oauth-token");
    assert!(auth.is_file());
    assert!(!auth.is_symlink());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&auth).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    replay.case(&server, "login-delete");
    setup::wait(&server, "adopted account refresh ends", |rows| {
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| row["busy"] == false)
    });
    replay.case(&server, "account-delete");
    let first = replay.case(&server, "login-start").json();
    let newer = server
        .call("POST", "/api/accounts/login", Some(json!({})))
        .json();
    assert_ne!(first["id"], newer["id"]);
    replay.case(&server, "login-poll-replaced");
    replay.case(&server, "login-submit-replaced");
    replay.case(&server, "login-delete");
    let cancelled = server.call(
        "DELETE",
        &format!("/api/accounts/login/{}", newer["id"].as_str().unwrap()),
        None,
    );
    assert_eq!(cancelled.status, 200);
    assert_eq!(cancelled.json(), json!({"ok":true}));
    assert!(!auth.exists());
    assert!(!server.root.join("browser-record.txt").exists());
    assert!(
        fs::read_dir(server.data.join("slots"))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".login-"))
    );
    setup::evidence(
        "login-contract",
        json!({"capturedCases":["login-start","login-poll","login-code-invalid","login-submit","login-delete","login-poll-replaced","login-submit-replaced","account-delete"],"privateCredentials":true,"temporaryHomesRemoved":true,"realBrowserCalls":0}),
    );
    server.stop();
}
#[test]
fn captured_settings_and_defaults_preserve_unknown_fields_and_validation() {
    let mut server = Server::new_stopped("settings-replay", true);
    setup::captured_home(&server);
    setup::write(
        &server.data.join("settings.json"),
        "{\"keep_awake\":\"off\",\"future\":{\"opaque\":123456789012345678901234567890}}",
    );
    let lease = antiquota_store::lock_file(&server.data.join("state/leases/one@example.com.lock"))
        .unwrap()
        .unwrap();
    server.start();
    let mut replay = Replay::new(&server);
    replay.case(&server, "settings");
    replay.case(&server, "defaults");
    replay.case(&server, "defaults-error");
    drop(lease);
    let stored: serde_json::Value =
        serde_json::from_slice(&fs::read(server.data.join("settings.json")).unwrap()).unwrap();
    assert_eq!(
        stored["future"]["opaque"].to_string(),
        "123456789012345678901234567890"
    );
    assert_eq!(stored["reserve"], "4");
    setup::evidence(
        "settings-contract",
        json!({"capturedCases":["settings","defaults","defaults-error"],"unknownLargeNumberPreserved":true,"stringSettings":true}),
    );
    server.stop();
}

#[test]
fn login_preserves_long_urls_and_rejects_codes_without_partial_accounts() {
    let mut server = Server::new_stopped("long-login", true);
    let url = format!(
        "https://accounts.google.com/o/oauth2/auth?client_id=stub&state={}",
        "s".repeat(600)
    );
    setup::script(
        &server,
        json!({"login":{"email":"three@example.com","url":url}}),
    );
    setup::start(&mut server);
    let link = server.call("POST", "/api/accounts/login", Some(json!({})));
    assert_eq!(link.status, 200);
    assert_eq!(link.json()["url"], url);
    let id = link.json()["id"].as_str().unwrap().to_string();
    let response = server.call(
        "POST",
        &format!("/api/accounts/login/{id}"),
        Some(json!({"code":"wrong-code"})),
    );
    assert_eq!(response.status, 200);
    assert_eq!(response.json()["state"], "failed");
    assert!(
        response.json()["error"]
            .as_str()
            .unwrap()
            .contains("authentication failed")
    );
    assert_eq!(response.json()["name"], serde_json::Value::Null);
    assert!(!server.data.join("slots/three@example.com").exists());
    assert!(
        fs::read_dir(server.data.join("slots"))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".login-"))
    );
    setup::evidence(
        "login-errors",
        json!({"longUrlLength":url.len(),"urlExact":true,"providerRejectionReported":true,"partialSlots":0,"temporaryHomesRemoved":true,"networkNavigation":false}),
    );
    server.stop();
}

#[test]
fn verified_login_keeps_the_account_parked_until_successful_probe() {
    let mut server = Server::new_stopped("verify-login", true);
    let url = "https://accounts.google.com/signin/continue?fixture=login";
    setup::script(
        &server,
        json!({"login":{"email":"three@example.com","verify_url":url},"metadata_by_slot":{"three@example.com":{"verify_url":url}}}),
    );
    setup::start(&mut server);
    let link = server
        .call("POST", "/api/accounts/login", Some(json!({})))
        .json();
    let id = link["id"].as_str().unwrap();
    let response = server.call(
        "POST",
        &format!("/api/accounts/login/{id}"),
        Some(json!({"code":"good-code"})),
    );
    assert_eq!(response.status, 200);
    assert_eq!(response.json()["state"], "verify");
    assert_eq!(response.json()["verify_url"], url);
    assert_eq!(response.json()["name"], "three@example.com");
    let accounts = server.call("GET", "/api/accounts", None).json();
    let account = accounts
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["slot"] == "three@example.com")
        .unwrap();
    assert_eq!(account["verify"], url);
    assert!(account["walls"]["gemini"].as_f64().unwrap() > 0.0);
    assert!(account["walls"]["cg"].as_f64().unwrap() > 0.0);
    assert!(
        server
            .data
            .join("slots/three@example.com/.gemini/antigravity-cli/antigravity-oauth-token")
            .is_file()
    );
    setup::wait(&server, "adopted verification refresh ends", |rows| {
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| row["busy"] == false)
    });
    setup::script(&server, json!({}));
    let response = server.call(
        "POST",
        "/api/accounts/three@example.com/check",
        Some(json!({})),
    );
    assert_eq!(response.status, 200);
    let verified = response.json();
    let account = verified
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["slot"] == "three@example.com")
        .unwrap();
    assert_eq!(account["verify"], serde_json::Value::Null);
    assert_eq!(account["walls"], json!({"gemini":0.0,"cg":0.0}));
    setup::evidence(
        "verify-login",
        json!({"identity":"three@example.com","tokenRetained":true,"verificationUrl":url,"bothBucketsParked":true,"probeRestoresAccount":true}),
    );
    server.stop();
}
