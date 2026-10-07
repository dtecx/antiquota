//! Proves metadata bounds and process-tree cleanup when pipes are inherited.
use crate::{setup, support::Server};
use serde_json::json;
use std::{
    fs, thread,
    time::{Duration, Instant},
};

#[test]
fn huge_metadata_fails_promptly_preserves_cache_and_releases_lease() {
    let mut server = Server::new_stopped("metadata-limit", true);
    setup::start(&mut server);
    setup::wait(&server, "initial metadata idle", |rows| {
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| row["busy"] == false)
    });
    let cache = server.data.join("state/quota/one@example.com.json");
    let before = fs::read(&cache).unwrap();
    let mut quota = setup::healthy_quota(0.9);
    quota["padding"] = json!("x".repeat(3_000_000));
    setup::script(
        &server,
        json!({"metadata_by_slot":{setup::ONE:{"quota":quota}}}),
    );
    let started = Instant::now();
    let response = server.call(
        "POST",
        "/api/accounts/one@example.com/check",
        Some(json!({})),
    );
    assert_eq!(response.status, 200);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "oversized metadata did not fail promptly"
    );
    assert_eq!(fs::read(cache).unwrap(), before);
    assert_eq!(response.json()[0]["busy"], false);
    setup::evidence(
        "metadata-limit",
        json!({"bytes":3_000_000,"boundedSeconds":2,"cachePreserved":true,"leaseReleased":true}),
    );
    server.stop();
}

#[test]
fn quota_probe_kills_descendants_after_parent_exits_with_inherited_pipes() {
    let mut server = Server::new_stopped("metadata-descendant", true);
    setup::start(&mut server);
    setup::wait(&server, "initial metadata idle", |rows| {
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| row["busy"] == false)
    });
    let marker = server.project.join("probe-descendant-marker");
    setup::script(
        &server,
        json!({"metadata_by_slot":{setup::ONE:{"inherited_pipe_ms":500,"descendant_marker":marker}}}),
    );
    let started = Instant::now();
    let response = server.call(
        "POST",
        "/api/accounts/one@example.com/check",
        Some(json!({})),
    );
    assert_eq!(response.status, 200);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(response.json()[0]["busy"], false);
    let until = Instant::now() + Duration::from_millis(650);
    while Instant::now() < until {
        assert!(
            !marker.exists(),
            "metadata descendant wrote after completed probe"
        );
        thread::sleep(Duration::from_millis(10));
    }
    setup::evidence(
        "metadata-descendant",
        json!({"parentExitHandled":true,"inheritedPipesBounded":true,"delayedWritePrevented":true,"leaseReleased":true}),
    );
    server.stop();
}

#[test]
fn stop_during_probe_kills_inherited_pipe_descendant_before_delayed_write() {
    let mut server = Server::new_stopped("metadata-descendant-stop", true);
    setup::start(&mut server);
    setup::wait(&server, "initial metadata idle", |rows| {
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| row["busy"] == false)
    });
    let marker = server.project.join("cancelled-probe-marker");
    let id = server.create();
    setup::script(
        &server,
        json!({"scenarios_by_slot":{setup::ONE:"quota"},"metadata_by_slot":{setup::ONE:{"quota_delay_ms":10000,"inherited_pipe_ms":500,"descendant_marker":marker}}}),
    );
    setup::send(&server, &id, "Cancel inherited probe pipes.");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if setup::calls(&server)
            .iter()
            .any(|row| row["argv"][0] == "--fake-inherited-pipes")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "probe descendant was not observed"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let started = Instant::now();
    assert_eq!(
        server
            .call("POST", &format!("/api/sessions/{id}/stop"), Some(json!({})))
            .status,
        200
    );
    loop {
        let session = server
            .call("GET", &format!("/api/sessions/{id}"), None)
            .json();
        let accounts = server.call("GET", "/api/accounts", None).json();
        if session["meta"]["status"] == "idle" && accounts[0]["busy"] == false {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "probe cancellation blocked"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let until = Instant::now() + Duration::from_millis(650);
    while Instant::now() < until {
        assert!(!marker.exists(), "probe descendant survived cancellation");
        thread::sleep(Duration::from_millis(10));
    }
    setup::evidence(
        "metadata-descendant-stop",
        json!({"boundedSeconds":2,"leaseReleased":true,"delayedWritePrevented":true}),
    );
    server.stop();
}
