//! Replays immutable account, login, routing, and defaults captures exactly.
use crate::{
    setup,
    support::{Server, http::Events, replay::Replay},
};
use serde_json::{Value, json};
use std::{
    fs,
    time::{Duration, Instant},
};

const POST_LOGIN_QUOTA: f64 = 0.63;

fn post_login_refreshed(event: &Value) -> bool {
    event["t"] == "accounts"
        && event["accounts"].as_array().is_some_and(|rows| {
            rows.len() == 3
                && rows.iter().all(|row| row["busy"] == false)
                && rows.iter().any(|row| {
                    row["slot"] == "three@example.com"
                        && row["quota"]["gemini"]["5h"][0] == POST_LOGIN_QUOTA
                })
        })
}

fn wait_for_post_login_refresh(events: &mut Events) -> Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let event = events.next();
        if post_login_refreshed(&event) {
            return event;
        }
        assert!(
            Instant::now() < deadline,
            "login refresh never completed: {event}"
        );
    }
}

#[test]
fn login_refresh_barrier_rejects_an_early_all_idle_adoption_announcement() {
    let mut event = json!({"t":"accounts","accounts":[
        {"slot":setup::ONE,"busy":false},
        {"slot":"three@example.com","busy":false,"quota":{"gemini":{"5h":[0.9,null]}}},
        {"slot":setup::TWO,"busy":false}
    ]});
    assert!(!post_login_refreshed(&event));
    event["accounts"][1]["quota"]["gemini"]["5h"][0] = json!(POST_LOGIN_QUOTA);
    event["accounts"][1]["busy"] = json!(true);
    assert!(!post_login_refreshed(&event));
    event["accounts"][1]["busy"] = json!(false);
    assert!(post_login_refreshed(&event));
}

#[test]
fn pause_defaults_to_on_rejects_unknown_accounts_and_broadcasts_the_response() {
    let mut server = Server::new_stopped("pause-route-edges", true);
    setup::start(&mut server);
    let mut events = Events::open(server.port, &server.token);
    for (body, paused) in [(json!({}), true), (json!({"on": false}), false)] {
        let response = server.call("POST", "/api/accounts/one@example.com/pause", Some(body));
        assert_eq!(response.status, 200);
        let accounts = response.json();
        assert_eq!(accounts[0]["paused"], paused);
        assert_eq!(
            server.data.join("state/paused").join(setup::ONE).is_file(),
            paused
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let event = events.next();
            if event["t"] == "accounts" && event["accounts"] == accounts {
                assert_eq!(event["sid"], Value::Null);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "account response was not broadcast: {event}"
            );
        }
    }
    let missing = server.call(
        "POST",
        "/api/accounts/missing@example.com/pause",
        Some(json!({})),
    );
    assert_eq!(missing.status, 404);
    assert!(missing.json()["error"].is_string());
    assert!(
        !server
            .data
            .join("state/paused/missing@example.com")
            .exists()
    );
    server.stop();
}

#[test]
fn routing_reads_requested_tails_in_stored_order_and_skips_malformed_lines() {
    let mut server = Server::new_stopped("routing-route-edges", true);
    setup::start(&mut server);
    let rows: Vec<_> = (0..4).map(|index| json!({
        "ts": 1800000000.0 + f64::from(index), "slot": setup::ONE, "event": "pick", "future": {"index": index}
    })).collect();
    let mut lines = "not JSON\n{\"truncated\":\n".to_owned();
    for row in &rows {
        lines.push_str(&format!("{row}\n"));
    }
    setup::write(&server.data.join("state/routing.jsonl"), &lines);
    for (path, expected) in [
        ("/api/routing", json!(rows)),
        ("/api/routing?n=60", json!(rows)),
        ("/api/routing?n=0", json!(rows)),
        ("/api/routing?n=2", json!(&rows[2..])),
        ("/api/routing?n=1", json!(&rows[3..])),
    ] {
        let response = server.call("GET", path, None);
        assert_eq!(response.status, 200);
        assert_eq!(response.json(), expected, "{path}");
    }
    assert_eq!(
        fs::read_to_string(server.data.join("state/routing.jsonl")).unwrap(),
        lines
    );
    server.stop();
}

#[test]
fn slow_probes_and_post_delete_refresh_return_before_the_metadata_finishes() {
    let mut server = Server::new_stopped("async-account-route-edges", true);
    setup::start(&mut server);
    setup::script(
        &server,
        json!({"metadata_by_slot": {
            setup::ONE: {"quota_delay_ms": 1500, "quota": setup::healthy_quota(0.6)},
            setup::TWO: {"quota_delay_ms": 1500, "quota": setup::healthy_quota(0.7)}
        }}),
    );
    let before = setup::calls(&server).len();
    let started = Instant::now();
    let response = server.call("POST", "/api/accounts/probe", Some(json!({})));
    assert_eq!(response.status, 200);
    assert_eq!(response.json(), json!({"ok": true}));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "probe blocked on metadata"
    );
    setup::wait(&server, "slow probe leases the first account", |rows| {
        rows[0]["busy"] == true
    });
    let probed = setup::wait(&server, "both slow probes complete", |rows| {
        rows[0]["quota"]["gemini"]["5h"][0] == 0.6
            && rows[1]["quota"]["gemini"]["5h"][0] == 0.7
            && rows
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["busy"] == false)
    });
    assert_eq!(probed.as_array().unwrap().len(), 2);
    let calls = setup::calls(&server);
    for slot in [setup::ONE, setup::TWO] {
        assert!(
            calls[before..]
                .iter()
                .any(|row| row["slot"] == slot && setup::argument(row, "-p") == Some("/quota"))
        );
    }
    setup::script(
        &server,
        json!({"metadata_by_slot": {
            setup::ONE: {"quota_delay_ms": 1500, "quota": setup::healthy_quota(0.5)}
        }}),
    );
    let before_delete = calls.len();
    let started = Instant::now();
    let deleted = server.call("DELETE", "/api/accounts/two@example.com", None);
    assert_eq!(deleted.status, 200);
    assert_eq!(deleted.json(), json!({"ok": true}));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "delete blocked on remaining metadata"
    );
    assert!(!server.data.join("slots").join(setup::TWO).exists());
    setup::wait(&server, "delete starts a remaining-account probe", |rows| {
        rows[0]["busy"] == true
    });
    let refreshed = setup::wait(&server, "remaining-account probe completes", |rows| {
        rows[0]["busy"] == false && rows[0]["quota"]["gemini"]["5h"][0] == 0.5
    });
    assert_eq!(refreshed.as_array().unwrap().len(), 1);
    let calls = setup::calls(&server);
    let probes: Vec<_> = calls[before_delete..]
        .iter()
        .filter(|row| setup::argument(row, "-p") == Some("/quota"))
        .collect();
    assert_eq!(probes.len(), 1);
    assert_eq!(probes[0]["slot"], setup::ONE);
    assert!(
        setup::turn_calls(&server).is_empty(),
        "metadata uses no model turns"
    );
    server.stop();
}

#[cfg(unix)]
#[test]
fn login_uses_an_empty_temporary_home_and_rejects_failed_start_and_unknown_links() {
    use std::{os::unix::fs::PermissionsExt, thread};
    let mut server = Server::new_stopped("login-route-edges", true);
    setup::captured_home(&server);
    setup::script(&server, json!({"login": {"email": "three@example.com"}}));
    let original = fs::read(server.home.join(".gemini/antigravity-cli/settings.json")).unwrap();
    let real = server.root.join("fake-agy-real");
    fs::rename(&server.agy, &real).unwrap();
    // Observe the fresh HOME before fake agy can create login files, then preserve its argv/TTY.
    setup::write(
        &server.agy,
        "#!/usr/bin/env python3\nimport json, os, sys\nfrom pathlib import Path\nhome = Path(os.environ['HOME'])\nif home.name.startswith('.login-'):\n    with open(os.environ['ANTIQUOTA_LOGIN_HOME_RECORD'], 'a') as record:\n        record.write(json.dumps({'home': str(home), 'userprofile': os.environ.get('USERPROFILE'), 'ssh': os.environ.get('SSH_CONNECTION'), 'entries': sorted(p.name for p in home.iterdir())}) + '\\n')\nreal = os.environ['ANTIQUOTA_FAKE_REAL']\nos.execv(real, [real, *sys.argv[1:]])\n",
    );
    fs::set_permissions(&server.agy, fs::Permissions::from_mode(0o700)).unwrap();
    let record = server.root.join("login-homes.jsonl");
    server.env.extend([
        (
            "ANTIQUOTA_LOGIN_HOME_RECORD".into(),
            record.to_string_lossy().into_owned(),
        ),
        (
            "ANTIQUOTA_FAKE_REAL".into(),
            real.to_string_lossy().into_owned(),
        ),
    ]);
    setup::start(&mut server);
    let unknown = server.call("GET", "/api/accounts/login/unknown", None);
    assert_eq!(unknown.status, 200);
    assert_eq!(
        unknown.json(),
        json!({"state": "failed", "error": "replaced by a newer sign-in link (only the newest one works)"})
    );
    let link = server.call("POST", "/api/accounts/login", Some(json!({})));
    assert_eq!(link.status, 200);
    let link = link.json();
    assert_eq!(link["seconds"], 60);
    assert!(
        link["url"]
            .as_str()
            .unwrap()
            .starts_with("https://accounts.google.com/")
    );
    let id = link["id"].as_str().unwrap();
    let observed: Value =
        serde_json::from_str(fs::read_to_string(&record).unwrap().lines().next().unwrap()).unwrap();
    let temporary = server.data.join("slots").join(format!(".login-{id}"));
    assert_eq!(observed["home"], temporary.to_string_lossy().as_ref());
    assert_eq!(observed["userprofile"], observed["home"]);
    assert_eq!(observed["ssh"], antiquota_accounts::FILE_TOKENS);
    assert_eq!(observed["entries"], json!([]));
    let rejected = server.call(
        "POST",
        &format!("/api/accounts/login/{id}"),
        Some(json!({"code": "wrong-code"})),
    );
    assert_eq!(rejected.status, 200);
    assert_eq!(rejected.json()["state"], "failed");
    let deadline = Instant::now() + Duration::from_secs(3);
    while temporary.exists() {
        assert!(
            Instant::now() < deadline,
            "rejected login HOME was retained"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!server.data.join("slots/three@example.com").exists());
    setup::script(
        &server,
        json!({"login": {"email": "three@example.com", "url": "https://example.invalid/not-a-google-url", "auto_complete_ms": 0, "error": "fixture authentication failed"}}),
    );
    let failed = server.call("POST", "/api/accounts/login", Some(json!({})));
    assert_eq!(failed.status, 400);
    assert!(failed.json()["error"].is_string());
    let homes: Vec<Value> = fs::read_to_string(&record)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(homes.len(), 2);
    assert_eq!(homes[1]["entries"], json!([]));
    let failed_home = std::path::Path::new(homes[1]["home"].as_str().unwrap());
    assert_eq!(
        failed_home.parent(),
        Some(server.data.join("slots").as_path())
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while failed_home.exists() {
        assert!(Instant::now() < deadline, "failed login HOME was retained");
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!server.data.join("slots/three@example.com").exists());
    assert_eq!(
        server
            .call("GET", "/api/accounts", None)
            .json()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        fs::read(server.home.join(".gemini/antigravity-cli/settings.json")).unwrap(),
        original
    );
    server.stop();
}

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
    let lease = setup::lease(&server, setup::ONE);
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
    setup::script(
        &server,
        json!({
            "login":{"email":"three@example.com"},
            "metadata_by_slot":{"three@example.com":{"quota":setup::healthy_quota(POST_LOGIN_QUOTA)}}
        }),
    );
    setup::start(&mut server);
    let mut events = Events::open(server.port, &server.token);
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
        assert_eq!(
            fs::metadata(server.data.join("slots"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    // Adoption reads default temporary-HOME quota; only the named-slot refresh publishes 63%.
    wait_for_post_login_refresh(&mut events);
    replay.case(&server, "login-delete");
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
fn delayed_post_login_refresh_keeps_busy_delete_exact_until_completion() {
    let mut server = Server::new_stopped("login-refresh-delete-overlap", true);
    setup::captured_home(&server);
    setup::script(
        &server,
        json!({
            "login":{"email":"three@example.com"},
            "metadata_by_slot":{"three@example.com":{
                "quota_delay_ms":1500,"quota":setup::healthy_quota(POST_LOGIN_QUOTA)
            }}
        }),
    );
    setup::start(&mut server);
    let mut events = Events::open(server.port, &server.token);
    let mut replay = Replay::new(&server);
    replay.case(&server, "login-start");
    replay.case(&server, "login-submit");
    let busy = setup::wait(
        &server,
        "named-slot post-login probe holds its real lease",
        |rows| {
            rows.as_array()
                .unwrap()
                .iter()
                .any(|row| row["slot"] == "three@example.com" && row["busy"] == true)
                && setup::calls(&server).iter().any(|call| {
                    call["slot"] == "three@example.com"
                        && setup::argument(call, "-p") == Some("/quota")
                })
        },
    );
    assert!(!post_login_refreshed(
        &json!({"t":"accounts","accounts":busy})
    ));
    assert!(setup::calls(&server).iter().any(|call| {
        call["slot"] == "three@example.com" && setup::argument(call, "-p") == Some("/quota")
    }));
    let auth = server
        .data
        .join("slots/three@example.com/.gemini/antigravity-cli/antigravity-oauth-token");
    let rejected = server.call("DELETE", "/api/accounts/three@example.com", None);
    assert_eq!(rejected.status, 400);
    assert_eq!(
        rejected.json(),
        json!({
            "error":"'three@example.com' is running a call; stop it first"
        })
    );
    assert!(auth.is_file());
    wait_for_post_login_refresh(&mut events);
    replay.case(&server, "login-delete");
    replay.case(&server, "account-delete");
    assert!(!auth.exists());
    server.evidence(
        "login-refresh-delete",
        &json!({
            "forcedProbeDelayMs":1500,"busyDeleteStatus":400,
            "completedRefreshQuota":POST_LOGIN_QUOTA,"capturedDeleteStatus":200,
            "retryCount":0,"paidCalls":0
        }),
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
