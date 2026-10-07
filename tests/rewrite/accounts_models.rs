//! Proves model discovery uses a leased signed-in account and retains a useful cache.
use crate::{setup, support::Server};
use serde_json::{Value, json};
use std::{
    fs, thread,
    time::{Duration, Instant},
};

#[test]
fn model_cache_is_used_while_first_account_busy_then_refreshes_from_that_account() {
    let mut server = Server::new_stopped("models-busy", true);
    let cache = server.data.join("state/models.json");
    let cached = json!([{"id":"gemini-3.8-flash-low","label":"Cached Flash"}]);
    setup::write(&cache, &cached.to_string());
    let models = json!([{"id":"gemini-3.8-flash-high","label":"Flash High"},{"id":"claude-opus","label":"Claude fixture"}]);
    setup::script(&server, json!({"metadata":{"models":models}}));
    let lease = antiquota_store::lock_file(&server.data.join("state/leases/one@example.com.lock"))
        .unwrap()
        .unwrap();
    server.start();
    assert_eq!(
        server.call("GET", "/api/state", None).json()["models"],
        cached
    );
    assert!(
        !setup::calls(&server)
            .iter()
            .any(|row| row["argv"][0] == "models")
    );
    drop(lease);
    assert_eq!(
        server
            .call("POST", "/api/accounts/probe", Some(json!({})))
            .status,
        200
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if server.call("GET", "/api/state", None).json()["models"] == models {
            break;
        }
        assert!(Instant::now() < deadline, "models were not refreshed");
        thread::sleep(Duration::from_millis(10));
    }
    let calls: Vec<_> = setup::calls(&server)
        .into_iter()
        .filter(|row| row["argv"][0] == "models")
        .collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["slot"], setup::ONE);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(cache).unwrap()).unwrap(),
        models
    );
    setup::evidence(
        "models-refresh",
        json!({"cachedWhileBusy":true,"refreshSlot":setup::ONE,"models":models}),
    );
    server.stop();
}

#[test]
fn empty_model_probe_keeps_cached_models_and_exact_file_bytes() {
    let mut server = Server::new_stopped("models-empty", true);
    let cache = server.data.join("state/models.json");
    let original = "[\n  {\"id\":\"gemini-3.8-flash-high\",\"label\":\"Useful cached model\"}\n]\n";
    setup::write(&cache, original);
    setup::script(&server, json!({"metadata":{"models":[]}}));
    setup::start(&mut server);
    assert_eq!(fs::read_to_string(cache).unwrap(), original);
    assert_eq!(
        server.call("GET", "/api/state", None).json()["models"],
        serde_json::from_str::<Value>(original).unwrap()
    );
    setup::evidence(
        "models-empty",
        json!({"emptyProbeDoesNotEraseCache":true,"fileBytesUnchanged":true}),
    );
    server.stop();
}
