//! Proves stack imports, editor limits and bounded project discovery through HTTP.
use crate::{setup, support::Server};
use serde_json::json;
use std::fs;
#[test]
fn stack_imports_copy_all_supported_resources_preserve_sources_and_report_toggle_failure() {
    let mut server = Server::new_stopped("parity-imports", false);
    setup::captured_home(&server);
    let cli = server.home.join(".gemini");
    let own = server.data.join("stack/.gemini");
    setup::write(
        &cli.join("config/hooks/scripts/helper.sh"),
        "printf helper\n",
    );
    setup::write(&cli.join("config/hooks.json"), &json!({"extra":{"command":cli.join("config/hooks/scripts/helper.sh").to_string_lossy(),"enabled":true}}).to_string());
    setup::write(
        &cli.join("config/mcp_config.json"),
        &json!({"mcpServers":{"extra":{"command":cli.join("config/tool").to_string_lossy()}}})
            .to_string(),
    );
    setup::write(
        &cli.join("antigravity-cli/mcp/extra/state.json"),
        "source state",
    );
    setup::write(
        &cli.join("antigravity-cli/plugin_data/example/data"),
        "plugin state",
    );
    setup::write(
        &cli.join("antigravity/skills/ide-skill/SKILL.md"),
        "IDE skill",
    );
    setup::write(&own.join("antigravity-cli/settings.json"), "{\"other\":42}");
    server.start();
    for (source, kind) in [
        ("cli", "plugin"),
        ("cli", "hook"),
        ("cli", "mcp"),
        ("cli", "skill"),
        ("cli", "rule"),
        ("cli", "gemini_md"),
        ("cli", "permissions"),
        ("ide", "skill"),
    ] {
        let response = server.call(
            "POST",
            "/api/settings/copy",
            Some(json!({"source":source,"kind":kind})),
        );
        assert_eq!(response.status, 200, "{source} {kind}: {}", response.json());
    }
    assert_eq!(
        fs::read_to_string(own.join("antigravity-cli/plugin_data/example/data")).unwrap(),
        "plugin state"
    );
    assert_eq!(
        fs::read_to_string(own.join("config/hooks/scripts/helper.sh")).unwrap(),
        "printf helper\n"
    );
    assert_eq!(
        server.call("GET", "/api/settings/hook/extra", None).json()["command"],
        own.join("config/hooks/scripts/helper.sh")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(
        server.call("GET", "/api/settings/mcp/extra", None).json()["command"],
        own.join("config/tool").to_string_lossy().as_ref()
    );
    assert_eq!(
        fs::read_to_string(own.join("antigravity-cli/mcp/extra/state.json")).unwrap(),
        "source state"
    );
    setup::write(
        &own.join("antigravity-cli/mcp/extra/state.json"),
        "own state",
    );
    assert_eq!(
        server
            .call(
                "POST",
                "/api/settings/copy",
                Some(json!({"source":"cli","kind":"mcp","name":"extra"}))
            )
            .status,
        200
    );
    assert_eq!(
        fs::read_to_string(own.join("antigravity-cli/mcp/extra/state.json")).unwrap(),
        "own state"
    );
    assert!(own.join("config/skills/ide-skill/SKILL.md").exists());
    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(own.join("antigravity-cli/settings.json")).unwrap())
            .unwrap();
    assert_eq!(settings["other"], 42);
    assert_eq!(settings["permissions"]["allow"], json!(["command(ls)"]));
    for (source, kind, name) in [
        ("ide", "plugin", "example"),
        ("ide", "everything", "example"),
        ("cli", "rule", "absent"),
        ("cli", "hook", ".hidden"),
        ("cli", "hook", "bad/name"),
    ] {
        assert_eq!(
            server
                .call(
                    "POST",
                    "/api/settings/copy",
                    Some(json!({"source":source,"kind":kind,"name":name}))
                )
                .status,
            400
        );
    }
    for kind in ["skill", "rule"] {
        assert_eq!(
            server
                .call(
                    "POST",
                    &format!("/api/settings/{kind}/example/enabled"),
                    Some(json!({"on":false}))
                )
                .status,
            400
        );
        assert_eq!(
            server
                .call(
                    "PUT",
                    &format!("/api/settings/{kind}/example"),
                    Some(json!({"value":{}}))
                )
                .status,
            400
        );
    }
    assert_eq!(
        server
            .call("PUT", "/api/settings/hook/extra", Some(json!({"value":[]})))
            .status,
        400
    );
    for kind in ["mcp", "plugin"] {
        let name = if kind == "mcp" { "extra" } else { "example" };
        assert_eq!(
            server
                .call(
                    "POST",
                    &format!("/api/settings/{kind}/{name}/enabled"),
                    Some(json!({"on":false}))
                )
                .status,
            200
        );
    }
    setup::script(&server, json!({"exit_code":3}));
    let failed = server.call(
        "POST",
        "/api/settings/plugin/example/enabled",
        Some(json!({"on":true})),
    );
    assert_eq!(failed.status, 400);
    assert!(failed.json()["error"].as_str().unwrap().contains("failed"));
    setup::script(&server, json!({}));
    for kind in ["plugin", "hook", "mcp"] {
        let name = if kind == "plugin" { "example" } else { "extra" };
        assert_eq!(
            server
                .call("DELETE", &format!("/api/settings/{kind}/{name}"), None)
                .status,
            200
        );
    }
    assert!(!own.join("antigravity-cli/plugin_data/example").exists());
    assert!(cli.join("config/plugins/example").exists());
    assert_eq!(
        fs::read_to_string(cli.join("antigravity-cli/mcp/extra/state.json")).unwrap(),
        "source state"
    );
    assert!(setup::turn_calls(&server).is_empty());
    server.stop();
}
#[test]
fn editor_limits_and_project_discovery_keep_private_and_heavy_paths_out() {
    let mut server = Server::new("parity-file-limits");
    let own = server.data.join("stack/.gemini");
    setup::write(&own.join("config/big.txt"), &"x".repeat(1_000_001));
    assert_eq!(
        server
            .call("GET", "/api/settings/file?path=config/big.txt", None)
            .status,
        400
    );
    fs::write(own.join("config/invalid.txt"), [255, b'a']).unwrap();
    assert_eq!(
        server
            .call("GET", "/api/settings/file?path=config/invalid.txt", None)
            .json()["text"],
        "�a"
    );
    assert_eq!(
        server
            .call(
                "PUT",
                "/api/settings/file",
                Some(json!({"path":"config/new/file","text":"x".repeat(1_000_001)}))
            )
            .status,
        400
    );
    assert_eq!(
        server
            .call(
                "PUT",
                "/api/settings/file",
                Some(json!({"path":"config/new/file","text":"界".repeat(1_000_000)}))
            )
            .status,
        200
    );
    assert_eq!(
        fs::read_to_string(own.join("config/new/file"))
            .unwrap()
            .chars()
            .count(),
        1_000_000
    );
    let id = server.create();
    for dir in [
        ".hidden",
        "node_modules",
        "__pycache__",
        "venv",
        "dist",
        "build",
    ] {
        setup::write(&server.project.join(dir).join("needle.txt"), "ignored");
    }
    setup::write(&server.project.join("long/needle.txt"), "included");
    setup::write(&server.project.join("needle/x.txt"), "included");
    let paths = server
        .call("GET", &format!("/api/sessions/{id}/files?q=NEEDLE"), None)
        .json();
    assert_eq!(paths, json!(["long/needle.txt", "needle/x.txt"]));
    for n in 0..405 {
        setup::write(&server.project.join(format!("hit{n:03}")), "");
    }
    let paths = server
        .call("GET", &format!("/api/sessions/{id}/files?q=hit"), None)
        .json();
    assert_eq!(paths.as_array().unwrap().len(), 30);
    assert_eq!(paths[0], "hit000");
    assert_eq!(paths[29], "hit029");
    for dir in ["zeta", "Alpha", ".secret"] {
        fs::create_dir_all(server.home.join(dir)).unwrap();
    }
    let picker = server.call("GET", "/api/fs?path=/missing", None).json();
    assert_eq!(picker["path"], server.home.to_string_lossy().as_ref());
    assert_eq!(picker["parent"], serde_json::Value::Null);
    let names = picker["dirs"].as_array().unwrap();
    assert!(!names.contains(&json!(".secret")));
    assert!(
        names.iter().position(|n| n == "Alpha").unwrap()
            < names.iter().position(|n| n == "zeta").unwrap()
    );
    server.stop();
}

#[test]
fn stack_reads_use_signed_in_slots_and_offline_folders_without_sign_in() {
    let mut server = Server::new_stopped("parity-probe-slot", false);
    setup::captured_home(&server);
    server.start();
    assert_eq!(server.call("GET", "/api/stack?refresh=1", None).status, 200);
    let calls: Vec<_> = setup::calls(&server)
        .into_iter()
        .filter(|row| matches!(setup::argument(row, "-p"), Some("/skills" | "/hooks")))
        .collect();
    assert_eq!(calls.len(), 2);
    for call in calls {
        assert_eq!(call["slot"], "one@example.com");
        assert!(
            !call["argv"]
                .as_array()
                .unwrap()
                .contains(&json!("--dangerously-skip-permissions"))
        );
    }
    server.stop();
    fs::remove_dir_all(server.data.join("slots")).unwrap();
    fs::write(server.root.join("fake-calls.jsonl"), "").unwrap();
    server.start();
    let before = setup::calls(&server).len();
    let info = server.call("GET", "/api/stack?refresh=1", None).json();
    assert_eq!(info["skills"], json!(["example"]));
    assert_eq!(info["hooks"], json!(["antiquota"]));
    assert_eq!(setup::calls(&server).len(), before);
    server.stop();
}
