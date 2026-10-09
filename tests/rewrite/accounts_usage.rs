//! Proves that the usage detail splits tokens into input, cached input, output and thinking.
use crate::{setup, support::Server};
use serde_json::json;

#[test]
fn usage_detail_splits_input_cached_input_output_and_thinking() {
    let mut server = Server::new_stopped("usage-detail", true);
    setup::script(
        &server,
        json!({"input_tokens":120,"output_tokens":30,"cache_read_tokens":480,"thinking_tokens":20}),
    );
    setup::start(&mut server);
    let id = server.create();
    setup::send(&server, &id, "Count my tokens.");
    let value = server.idle(&id);
    let end = value["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["t"] == "turn_end")
        .unwrap()
        .clone();
    assert_eq!(end["usage"]["cache_read_tokens"], 480);
    assert_eq!(end["usage"]["thinking_tokens"], 20);
    // The context counts the cached part of the prompt as well.
    assert_eq!(end["usage"]["context"], 600);
    let plain = server.call("GET", "/api/usage?days=7", None).json();
    assert!(plain.get("detail").is_none());
    let usage = server
        .call("GET", "/api/usage?days=7&detail=1", None)
        .json();
    assert_eq!(usage["days"], plain["days"]);
    let parts = json!({"input":120,"output":30,"thinking":20,"cache":480,"turns":1});
    let detail = &usage["detail"];
    for value in [
        &detail["period"],
        &detail["today"],
        &detail["days"][6],
        &detail["accounts"][setup::ONE],
        &detail["models"]["gemini-3.8-flash-high"],
    ] {
        assert_eq!(value, &parts);
    }
    let today = &server.call("GET", "/api/accounts", None).json()[0]["today"];
    assert_eq!(
        today,
        &json!({"input":120,"output":30,"thinking":20,"turns":1,"cache":480})
    );
    setup::evidence(
        "usage-detail",
        json!({"input":120,"cachedInput":480,"output":30,"thinking":20,"plainResponseUnchanged":true}),
    );
    server.stop();
}
