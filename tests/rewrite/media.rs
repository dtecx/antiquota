//! Proves attachments and dictation through the real engine and fake agy.
use crate::support::{Server, http, replay::Replay};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

fn raw(
    server: &Server,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> http::Response {
    let mut map: BTreeMap<String, String> =
        BTreeMap::from([("X-Antiquota-Token".into(), server.token.clone())]);
    for (k, v) in headers {
        map.insert((*k).into(), (*v).into());
    }
    http::request(server.port, method, path, &map, body)
}

#[test]
fn captured_attachment_routes_with_the_decoding_correction() {
    let mut server = Server::new("media-attach");
    let id = server.create();
    let mut replay = Replay::new(&server);
    replay.bind_setup("{{SESSION_1}}", &id);
    replay.case(&server, "attachment-upload-plain");
    replay.case(&server, "attachment-upload");
    replay.case(&server, "attachment-download-plain");
    replay.case(&server, "attachment-download-header-only");
    replay.case(&server, "attachment-download-missing");
    replay.case(&server, "attachment-limit");
    // B-021: the encoded name of an uploaded file with a space now downloads (the capture
    // recorded the legacy 404).
    let spaced = server.call(
        "GET",
        &format!(
            "/api/sessions/{id}/attachments/contract%20note.txt?token={}",
            server.token
        ),
        None,
    );
    assert_eq!(spaced.status, 200);
    assert_eq!(spaced.body, b"Fixture attachment.\n");
    let again = raw(
        &server,
        "POST",
        &format!("/api/sessions/{id}/attach"),
        &[("X-Filename", "fixture.txt")],
        b"x",
    );
    assert_eq!(again.json()["name"], "fixture-2.txt");
    let image = raw(
        &server,
        "POST",
        &format!("/api/sessions/{id}/attach"),
        &[("X-Filename", "../Shot.PNG")],
        b"\x89PNG",
    );
    let image = image.json();
    assert_eq!(
        (image["name"].clone(), image["image"].clone()),
        (json!("Shot.PNG"), json!(true))
    );
    let served = server.call(
        "GET",
        &format!(
            "/api/sessions/{id}/attachments/Shot.PNG?token={}",
            server.token
        ),
        None,
    );
    assert_eq!(served.headers["content-type"], "image/png");
    raw(
        &server,
        "POST",
        &format!("/api/sessions/{id}/attach"),
        &[("X-Filename", "blob.xyz")],
        b"?",
    );
    let unknown = server.call(
        "GET",
        &format!(
            "/api/sessions/{id}/attachments/blob.xyz?token={}",
            server.token
        ),
        None,
    );
    assert_eq!(unknown.headers["content-type"], "application/octet-stream");
    let escape = server.call(
        "GET",
        &format!(
            "/api/sessions/{id}/attachments/..%2Fmeta.json?token={}",
            server.token
        ),
        None,
    );
    assert_eq!(escape.status, 404);
    let missing = raw(&server, "POST", "/api/sessions/nope/attach", &[], b"x");
    assert_eq!(missing.status, 404);
    // An attachment-only message reaches agy with the file list and folder access.
    let path = image["path"].as_str().unwrap().to_owned();
    let sent = server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text": "", "attachments": [path.clone(), "/etc/passwd"]})),
    );
    assert_eq!(sent.json(), json!({"result": "started"}));
    let done = server.idle(&id);
    let user = done["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["t"] == "user")
        .unwrap()
        .clone();
    assert_eq!(
        user["attachments"],
        json!([{"name": "Shot.PNG", "path": path, "image": true}])
    );
    let call: Value = fs::read_to_string(server.root.join("fake-calls.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|c| {
            c["argv"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "stream-json")
        })
        .unwrap();
    let argv: Vec<&str> = call["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    let prompt = argv[argv.iter().position(|a| *a == "-p").unwrap() + 1];
    assert!(prompt.ends_with(&format!(
        "\n\nAttached files (open each with your file viewer; images are visible to you):\n- {path}"
    )));
    assert!(
        !prompt.contains("/etc/passwd"),
        "files outside the attachment folder are dropped"
    );
    let folder = server
        .data
        .join("sessions")
        .join(&id)
        .join("attachments")
        .to_string_lossy()
        .into_owned();
    assert!(argv.windows(2).any(|w| w == ["--add-dir", folder.as_str()]));
    server.stop();
}

#[test]
fn dictation_transcribes_on_a_gemini_account_and_cleans_up() {
    let mut server = Server::new_stopped("media-dictation", false);
    server
        .env
        .push(("FAKE_AGY_TRANSCRIPT".into(), "Captured speech.".into()));
    server.start();
    let id = server.create();
    let mut replay = Replay::new(&server);
    replay.bind_setup("{{SESSION_1}}", &id);
    replay.case(&server, "transcribe");
    replay.case(&server, "transcribe-empty");
    replay.case(&server, "transcribe-limit");
    let calls: Vec<Value> = fs::read_to_string(server.root.join("fake-calls.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .filter(|c: &Value| {
            c["argv"].as_array().unwrap().iter().any(|a| {
                a.as_str()
                    .unwrap_or("")
                    .starts_with("Listen to the audio file ")
            })
        })
        .collect();
    assert_eq!(calls.len(), 1);
    let argv: Vec<&str> = calls[0]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert!(
        argv.windows(2)
            .any(|w| w == ["--model", "gemini-3.8-flash-high"])
    );
    assert!(!argv.contains(&"--dangerously-skip-permissions"));
    assert!(argv.windows(2).any(|w| w == ["--output-format", "json"]));
    let folder = server.data.join("state/dictation");
    assert!(
        argv.windows(2)
            .any(|w| w[0] == "--add-dir" && w[1] == folder.to_string_lossy())
    );
    assert_eq!(
        fs::read_dir(&folder).unwrap().count(),
        0,
        "the recording is deleted"
    );
    let state = server.call("GET", "/api/state", None).json();
    assert_eq!(
        state["sessions"][0]["turns"], 0,
        "dictation starts no conversation"
    );
    // The only account was released: a second recording transcribes at once.
    let again = raw(
        &server,
        "POST",
        "/api/transcribe",
        &[("X-Audio-Type", "webm")],
        b"OggS",
    );
    assert_eq!(again.json()["text"], "Captured speech.");
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 0);
    server.stop();
    let mut quiet = Server::new("media-no-speech");
    let other = quiet.create();
    let mut replay = Replay::new(&quiet);
    replay.bind_setup("{{SESSION_1}}", &other);
    replay.case(&quiet, "transcribe-no-speech");
    quiet.stop();
}

#[test]
fn queued_attachments_wait_for_the_turn_and_keep_their_metadata() {
    let mut server = Server::new("media-queue");
    let id = server.create();
    let upload = raw(
        &server,
        "POST",
        &format!("/api/sessions/{id}/attach"),
        &[("X-Filename", "later.md")],
        b"# Later\n",
    )
    .json();
    let path = upload["path"].as_str().unwrap().to_owned();
    fs::write(
        &server.script,
        json!({"scenario":"delay","delay_ms":400}).to_string(),
    )
    .unwrap();
    let send = |body: Value| {
        server
            .call("POST", &format!("/api/sessions/{id}/send"), Some(body))
            .json()["result"]
            .clone()
    };
    assert_eq!(send(json!({"text": "First."})), "started");
    assert_eq!(
        send(json!({"text": "", "attachments": [path.clone()]})),
        "queued"
    );
    let busy = server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json();
    assert_eq!(
        busy["meta"]["queue"][0]["attachments"],
        json!([path.clone()]),
        "the queue keeps the structured attachment"
    );
    let done = server.idle_turns(&id, 2);
    let users: Vec<Value> = done["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["t"] == "user")
        .cloned()
        .collect();
    assert_eq!(users.len(), 2);
    assert_eq!(
        users[1]["attachments"],
        json!([{"name": "later.md", "path": path, "image": false}])
    );
    let turns: Vec<Vec<String>> = fs::read_to_string(server.root.join("fake-calls.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|c| {
            c["argv"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "stream-json")
        })
        .map(|c| {
            c["argv"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap().to_owned())
                .collect()
        })
        .collect();
    assert_eq!(turns.len(), 2);
    let second = &turns[1];
    let prompt = &second[second.iter().position(|a| a == "-p").unwrap() + 1];
    assert!(prompt.contains(&format!("images are visible to you):\n- {path}")));
    assert!(second.iter().any(|a| a == "--add-dir"));
    server.stop();
}

#[test]
fn untitled_sessions_take_the_first_line_or_the_attachment_fallback() {
    let mut server = Server::new("media-titles");
    let create = || {
        server
            .call(
                "POST",
                "/api/sessions",
                Some(json!({"cwd": server.project, "title": "", "model": "gemini-3.8-flash-high"})),
            )
            .json()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let titled = |id: &str, body: Value| {
        assert_eq!(
            server
                .call("POST", &format!("/api/sessions/{id}/send"), Some(body))
                .json()["result"],
            "started"
        );
        server.idle(id)["meta"]["title"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let first = create();
    let long = "x".repeat(70);
    assert_eq!(
        titled(
            &first,
            json!({"text": format!("\n\n  {long}\nsecond line")})
        ),
        "x".repeat(60)
    );
    // A later message keeps the first title.
    assert_eq!(
        titled(&first, json!({"text": "Another request"})),
        "x".repeat(60)
    );
    let second = create();
    let upload = raw(
        &server,
        "POST",
        &format!("/api/sessions/{second}/attach"),
        &[("X-Filename", "only.png")],
        b"\x89PNG",
    )
    .json();
    assert_eq!(
        titled(
            &second,
            json!({"text": "", "attachments": [upload["path"].clone()]})
        ),
        "(attachment)"
    );
    server.stop();
}
