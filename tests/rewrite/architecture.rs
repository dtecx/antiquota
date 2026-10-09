//! Proves the workspace layout, crate boundaries, code rules and architecture documents.
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn metadata() -> Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(repo())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn packages(meta: &Value) -> Vec<&Value> {
    meta["packages"].as_array().unwrap().iter().collect()
}
fn package<'a>(meta: &'a Value, name: &str) -> &'a Value {
    packages(meta)
        .into_iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("missing package {name}"))
}
/// Dependency names of one kind: `None` for normal, or "dev" or "build".
fn dependencies(package: &Value, kind: Option<&str>) -> Vec<String> {
    package["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["kind"].as_str() == kind)
        .map(|d| d["name"].as_str().unwrap().to_owned())
        .collect()
}
fn manifest_dir(package: &Value) -> PathBuf {
    PathBuf::from(package["manifest_path"].as_str().unwrap())
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
}
/// Every file below `dir` with one of the extensions, leaving out generated and installed files.
fn files(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut found = vec![];
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            if !["node_modules", "gen", "target", "dist", "gen-schemas"].contains(&name.as_str()) {
                found.extend(files(&path, extensions));
            }
        } else if extensions
            .iter()
            .any(|e| path.extension().is_some_and(|x| x == *e))
        {
            found.push(path);
        }
    }
    found.sort();
    found
}
/// Whether a source file only holds tests.
fn test_file(path: &Path) -> bool {
    let name = path.file_name().unwrap().to_string_lossy();
    name.contains("test")
        || name.contains(".spec.")
        || path
            .components()
            .any(|c| c.as_os_str() == "tests" || c.as_os_str() == "e2e")
}
/// The production text of a Rust file: test files and inline test modules are left out.
fn production(path: &Path) -> Option<String> {
    if test_file(path) {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    Some(match text.find("#[cfg(test)]") {
        Some(at) => text[..at].to_owned(),
        None => text,
    })
}
const MEMBERS: [(&str, &str); 11] = [
    ("antiquota-core", "crates/core"),
    ("antiquota-features", "crates/features"),
    ("antiquota-app", "crates/app"),
    ("antiquota-server", "crates/server"),
    ("antiquota-cli", "crates/cli"),
    ("antiquota-agy", "crates/adapters/agy"),
    ("antiquota-accounts", "crates/adapters/accounts"),
    ("antiquota-store", "crates/adapters/store"),
    ("antiquota-pty", "crates/adapters/pty"),
    ("antiquota-shell", "crates/adapters/shell"),
    ("antiquota-git", "crates/adapters/git"),
];
const ADAPTERS: [&str; 6] = [
    "antiquota-agy",
    "antiquota-accounts",
    "antiquota-store",
    "antiquota-pty",
    "antiquota-shell",
    "antiquota-git",
];

#[test]
fn the_workspace_has_core_features_app_adapter_server_and_cli_crates() {
    let meta = metadata();
    let root = repo();
    for (name, dir) in MEMBERS {
        assert_eq!(
            manifest_dir(package(&meta, name)),
            root.join(dir).canonicalize().unwrap(),
            "{name}"
        );
    }
    let ours: Vec<&str> = MEMBERS.iter().map(|(name, _)| *name).collect();
    let inner = |name: &str| -> Vec<String> {
        dependencies(package(&meta, name), None)
            .into_iter()
            .filter(|d| ours.contains(&d.as_str()))
            .collect()
    };
    // Dependencies point inward: core needs nothing of ours, adapters and features need only
    // core (the shell adapter also uses the store's file helpers), and only the outer layers
    // reach the application.
    assert!(inner("antiquota-core").is_empty());
    assert_eq!(inner("antiquota-features"), ["antiquota-core"]);
    for adapter in ADAPTERS {
        for dependency in inner(adapter) {
            assert!(
                dependency == "antiquota-core" || ADAPTERS.contains(&dependency.as_str()),
                "{adapter} depends on {dependency}"
            );
        }
    }
    assert!(inner("antiquota-app").contains(&"antiquota-core".to_owned()));
    assert!(inner("antiquota-server").contains(&"antiquota-app".to_owned()));
    assert!(inner("antiquota-cli").contains(&"antiquota-server".to_owned()));
}

#[test]
fn core_does_no_input_output_and_its_tests_need_no_adapter() {
    let meta = metadata();
    let core = package(&meta, "antiquota-core");
    let ours: Vec<&str> = MEMBERS.iter().map(|(name, _)| *name).collect();
    for kind in [None, Some("dev"), Some("build")] {
        for dependency in dependencies(core, kind) {
            assert!(
                !ours.contains(&dependency.as_str()),
                "core depends on {dependency}"
            );
            for io in [
                "tokio",
                "portable-pty",
                "process-wrap",
                "axum",
                "reqwest",
                "chrono",
            ] {
                assert_ne!(dependency, io, "core depends on {io}");
            }
        }
    }
    for file in files(&manifest_dir(core).join("src"), &["rs"]) {
        let Some(text) = production(&file) else {
            continue;
        };
        for io in [
            "std::fs",
            "std::process",
            "std::net",
            "std::env",
            "std::thread",
            "File::",
            "Command::new",
            "SystemTime",
        ] {
            assert!(!text.contains(io), "{} uses {io}", file.display());
        }
    }
}

#[test]
fn one_event_enum_feeds_the_ui_cli_and_approval_consumers() {
    let root = repo();
    let mut declarations = vec![];
    for file in files(&root.join("crates"), &["rs"]) {
        if fs::read_to_string(&file)
            .unwrap()
            .contains("pub enum Event {")
        {
            declarations.push(file);
        }
    }
    assert_eq!(declarations, [root.join("crates/core/src/event.rs")]);
    // The UI decodes the generated stream type; approval cards are among its variants.
    let generated = fs::read_to_string(root.join("ui/src/gen/StreamEvent.ts")).unwrap();
    for variant in [
        "\"user\"",
        "\"turn_start\"",
        "\"tool\"",
        "\"turn_end\"",
        "\"approval\"",
    ] {
        assert!(generated.contains(variant), "{variant}");
    }
    let stream = fs::read_to_string(root.join("ui/src/lib/stream.ts")).unwrap();
    assert!(stream.contains("from '../gen/StreamEvent'") && stream.contains("parseStreamEvent("));
    let approvals = fs::read_to_string(root.join("ui/src/lib/approvals.ts")).unwrap();
    assert!(approvals.contains("../gen/"));
    // The CLI decodes the same Rust type instead of reading raw JSON fields.
    let follow = fs::read_to_string(root.join("crates/cli/src/follow.rs")).unwrap();
    assert!(follow.contains("serde_json::from_str::<StreamEvent>"));
    for file in ["goal.rs", "follow.rs"] {
        let text = fs::read_to_string(root.join("crates/cli/src").join(file)).unwrap();
        assert!(!text.contains("[\"t\"]"), "{file} reads raw event fields");
    }
}

#[test]
fn unsafe_code_is_forbidden_in_every_crate() {
    let root = repo();
    let workspace = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(workspace.contains("[workspace.lints.rust]\nunsafe_code = \"forbid\""));
    for package in packages(&metadata()) {
        let dir = manifest_dir(package);
        let manifest = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(
            manifest.contains("[lints]\nworkspace = true"),
            "{} does not inherit the workspace lints",
            package["name"]
        );
        for file in files(&dir, &["rs"]) {
            let text = fs::read_to_string(&file).unwrap();
            for marker in ["unsafe {", "unsafe fn", "unsafe impl", "unsafe_code"] {
                assert!(!text.contains(marker), "{} has {marker}", file.display());
            }
        }
    }
}

#[test]
fn libraries_use_thiserror_tracing_and_serde_and_only_executables_use_anyhow() {
    let meta = metadata();
    for package in packages(&meta) {
        let name = package["name"].as_str().unwrap();
        let normal = dependencies(package, None);
        let executable_only = package["targets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["kind"].as_array().unwrap().iter().any(|k| k == "bin"));
        if normal.iter().any(|d| d == "anyhow") {
            assert!(executable_only, "the library {name} depends on anyhow");
        }
        for logger in ["log", "env_logger", "simplelog"] {
            assert!(!normal.iter().any(|d| d == logger), "{name} uses {logger}");
        }
        for file in files(&manifest_dir(package), &["rs"]) {
            let Some(text) = production(&file) else {
                continue;
            };
            assert!(
                !text.contains("impl std::error::Error for") && !text.contains("impl Error for"),
                "{} implements an error by hand",
                file.display()
            );
            let lines: Vec<&str> = text.lines().collect();
            for (at, line) in lines.iter().enumerate() {
                let declares = line.trim_start().starts_with("pub enum ") && line.contains("Error");
                if declares {
                    let derive = lines[at.saturating_sub(2)..at].join(" ");
                    assert!(
                        derive.contains("thiserror::Error")
                            || (derive.contains("Error") && text.contains("use thiserror::Error")),
                        "{} declares {line} without thiserror",
                        file.display()
                    );
                }
            }
        }
    }
    // Libraries log through tracing; the only printed lines are the serve banner.
    for name in [
        "antiquota-app",
        "antiquota-server",
        "antiquota-cli",
        "antiquota-agy",
    ] {
        assert!(dependencies(package(&meta, name), None).contains(&"tracing".to_owned()));
    }
    for (name, dir) in MEMBERS {
        if name == "antiquota-cli" {
            continue;
        }
        for file in files(&repo().join(dir).join("src"), &["rs"]) {
            let Some(text) = production(&file) else {
                continue;
            };
            let prints = text.matches("println!(").count() + text.matches("eprintln!(").count();
            let banner = file.ends_with("crates/server/src/runtime.rs");
            assert_eq!(prints, if banner { 2 } else { 0 }, "{}", file.display());
        }
    }
    // Every wire type derives serde, so the generated UI types and the JSON agree.
    let api = fs::read_to_string(repo().join("crates/server/src/api.rs")).unwrap();
    let lines: Vec<&str> = api.lines().collect();
    for (at, line) in lines.iter().enumerate() {
        if line.starts_with("pub struct ") || line.starts_with("pub enum ") {
            let derive = lines[at.saturating_sub(3)..at].join(" ");
            assert!(
                derive.contains("Serialize") && derive.contains("TS"),
                "{line} lacks serde"
            );
        }
    }
}

#[test]
fn modules_stay_small_and_state_what_they_own() {
    let root = repo();
    let mut sources = vec![];
    for dir in ["crates", "app/src-tauri/src", "tests/fake-agy/src"] {
        sources.extend(files(&root.join(dir), &["rs"]));
    }
    sources.extend(files(&root.join("ui/src"), &["ts", "svelte"]));
    let mut long = vec![];
    for file in sources {
        if test_file(&file) || file.ends_with("build.rs") {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap();
        let first = text.lines().next().unwrap_or("");
        let owned = match file.extension().and_then(|e| e.to_str()) {
            Some("rs") => first.starts_with("//!"),
            Some("svelte") => first.starts_with("<!--"),
            _ => first.starts_with("/**") || first.starts_with("//"),
        };
        assert!(owned, "{} has no ownership comment", file.display());
        // "Near or below 400 lines": no product module may pass 500.
        if text.lines().count() > 500 {
            long.push(file);
        }
    }
    assert!(long.is_empty(), "modules over 500 lines: {long:?}");
}

#[test]
fn every_crate_has_tests_of_its_own() {
    for package in packages(&metadata()) {
        let mut count = 0;
        for file in files(&manifest_dir(package), &["rs"]) {
            let text = fs::read_to_string(&file).unwrap();
            count += text.matches("#[test]").count()
                + text.matches("#[tokio::test]").count()
                + text.matches("proptest! {").count();
        }
        assert!(count >= 2, "{} has {count} tests", package["name"]);
    }
}

#[test]
fn the_architecture_document_draws_every_layer_and_decisions_stay_short() {
    let root = repo();
    let doc = fs::read_to_string(root.join("docs/ARCHITECTURE.md")).unwrap();
    let start = doc.find("```mermaid").expect("a mermaid diagram");
    let diagram = &doc[start..start + doc[start..].find("\n```\n").unwrap()];
    for part in [
        "UI", "Shell", "CLI", "Server", "App", "Features", "Core", "agy", "accounts", "store",
        "pty", "shell", "git",
    ] {
        assert!(diagram.contains(part), "the diagram lacks {part}");
    }
    for section in [
        "## Dependencies and ownership",
        "## Ports and feature hooks",
        "## UI and native shell",
    ] {
        assert!(doc.contains(section), "{section}");
    }
    // The ownership table names every crate folder, and only folders that exist.
    for (_, dir) in MEMBERS {
        assert!(
            doc.contains(&format!("| `{dir}` |")),
            "the table lacks {dir}"
        );
    }
    for row in doc.lines().filter(|l| l.starts_with("| `crates/")) {
        let dir = row.split('`').nth(1).unwrap();
        assert!(root.join(dir).is_dir(), "the table names a missing {dir}");
    }
    let adrs = files(&root.join("docs/adr"), &["md"]);
    assert!(adrs.len() >= 26);
    for adr in adrs {
        let text = fs::read_to_string(&adr).unwrap();
        assert!(text.starts_with("# "), "{}", adr.display());
        assert!(text.lines().count() <= 40, "{} is long", adr.display());
    }
}
