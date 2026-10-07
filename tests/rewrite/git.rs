//! Proves worktree sessions, change review and project files through the real engine.
use crate::support::{Server, replay::Replay};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Makes the server's project a repository with one commit on main.
fn repository(server: &Server) {
    let p = &server.project;
    git(p, &["init", "-q", "-b", "main"]);
    for (k, v) in [
        ("user.email", "test@example.com"),
        ("user.name", "Test"),
        ("commit.gpgsign", "false"),
    ] {
        git(p, &["config", k, v]);
    }
    fs::write(p.join("readme.txt"), "Fixture repository.\n").unwrap();
    git(p, &["add", "."]);
    git(p, &["commit", "-q", "-m", "first"]);
}

fn json_call(server: &Server, method: &str, path: &str, body: Option<Value>) -> Value {
    let response = server.call(method, path, body);
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    response.json()
}

#[test]
fn captured_worktree_flow_and_refusals() {
    let mut server = Server::new("git-captured");
    repository(&server);
    let id = server.create();
    let mut replay = Replay::new(&server);
    replay.bind_setup("{{SESSION_1}}", &id);
    replay.case(&server, "session-file-outside");
    replay.case(&server, "session-tree-outside");
    replay.case(&server, "session-git-merge-no-worktree");
    replay.case(&server, "session-git-remove-no-worktree");
    let created = replay.case(&server, "worktree-create").json();
    let sid = created["id"].as_str().unwrap();
    let branch = created["worktree"]["branch"].as_str().unwrap();
    assert_eq!(branch, format!("antiquota/worktree-fixture-{}", &sid[..4]));
    let folder = Path::new(created["cwd"].as_str().unwrap());
    assert!(!folder.join("notes.txt").exists());
    fs::write(folder.join("worktree.txt"), "Worktree change.\n").unwrap();
    replay.case(&server, "worktree-git");
    replay.case(&server, "worktree-commit");
    replay.case(&server, "worktree-committed-diff");
    replay.case(&server, "worktree-merge");
    assert_eq!(
        fs::read_to_string(server.project.join("worktree.txt")).unwrap(),
        "Worktree change.\n",
        "the merge reached the main checkout"
    );
    replay.case(&server, "worktree-remove");
    assert!(!folder.exists());
    assert!(
        git(&server.project, &["branch", "--list", branch])
            .trim()
            .is_empty()
    );
    server.stop();
}

#[test]
fn untitled_worktree_branches_take_the_first_request_and_removal_tells_agy() {
    let mut server = Server::new("git-branch");
    repository(&server);
    let created = json_call(
        &server,
        "POST",
        "/api/sessions",
        Some(json!({"cwd": server.project, "worktree": true})),
    );
    let id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(
        created["worktree"]["branch"],
        format!("antiquota/session-{}", &id[..4])
    );
    json_call(
        &server,
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text": "Fix the login bug"})),
    );
    let done = server.idle(&id);
    let renamed = format!("antiquota/fix-the-login-bug-{}", &id[..4]);
    assert_eq!(done["meta"]["worktree"]["branch"], renamed);
    let folder = Path::new(done["meta"]["cwd"].as_str().unwrap()).to_path_buf();
    assert_eq!(
        git(&folder, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        renamed
    );
    let removed = json_call(
        &server,
        "POST",
        &format!("/api/sessions/{id}/git/remove"),
        Some(json!({})),
    );
    assert_eq!(removed["archived"], true);
    assert_eq!(removed["worktree"], Value::Null);
    assert!(
        !git(&server.project, &["branch", "--list", &renamed])
            .trim()
            .is_empty(),
        "the branch stays"
    );
    json_call(
        &server,
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text": "Carry on."})),
    );
    server.idle_turns(&id, 2);
    let calls: Vec<Value> = fs::read_to_string(server.root.join("fake-calls.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .filter(|c: &Value| {
            c["argv"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "stream-json")
        })
        .collect();
    let last = calls.last().unwrap();
    let prompt = last["argv"]
        .as_array()
        .unwrap()
        .windows(2)
        .find(|w| w[0] == "-p")
        .unwrap()[1]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(prompt.contains(
        "This session's worktree was removed; you now work in the main checkout.\n\nCarry on."
    ));
    assert_eq!(
        last["cwd"],
        server
            .project
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    let after = json_call(&server, "GET", &format!("/api/sessions/{id}"), None);
    assert_eq!(
        after["meta"]["pending_note"], "",
        "the note is delivered once"
    );
    server.stop();
}

#[test]
fn review_tree_and_files_of_the_main_checkout() {
    let mut server = Server::new("git-review");
    repository(&server);
    let id = server.create();
    fs::write(server.project.join("readme.txt"), "Changed.\nTwice.\n").unwrap();
    fs::write(server.project.join("new.txt"), "a\nb\nc\n").unwrap();
    fs::create_dir_all(server.project.join("src")).unwrap();
    let status = json_call(&server, "GET", &format!("/api/sessions/{id}/git"), None);
    assert_eq!(status["branch"], "main");
    assert_eq!(status["worktree"], Value::Null);
    assert_eq!(
        status["files"],
        json!([
            {"path": "readme.txt", "code": "M", "add": 2, "del": 1},
            {"path": "new.txt", "code": "??", "add": 3, "del": 0}
        ])
    );
    let diff = json_call(
        &server,
        "GET",
        &format!("/api/sessions/{id}/git/diff?path=new.txt"),
        None,
    );
    assert!(
        diff["diff"]
            .as_str()
            .unwrap()
            .contains("+++ b/new.txt\n@@ -0,0 +1,3 @@")
    );
    let refused = server.call(
        "GET",
        &format!("/api/sessions/{id}/git/diff?committed=1"),
        None,
    );
    assert_eq!(
        refused.json()["error"],
        "only worktree sessions have a base to compare with"
    );
    json_call(
        &server,
        "POST",
        &format!("/api/sessions/{id}/git/discard"),
        Some(json!({"path": "readme.txt"})),
    );
    assert_eq!(
        fs::read_to_string(server.project.join("readme.txt")).unwrap(),
        "Fixture repository.\n"
    );
    let bad = server.call(
        "POST",
        &format!("/api/sessions/{id}/git/discard"),
        Some(json!({"path": "../outside"})),
    );
    assert_eq!(bad.json()["error"], "pick a file inside the repository");
    let commit = json_call(
        &server,
        "POST",
        &format!("/api/sessions/{id}/git/commit"),
        Some(json!({"message": "Add new.txt\n\nDetails."})),
    );
    let sha = commit["sha"].as_str().unwrap();
    let nothing = server.call(
        "POST",
        &format!("/api/sessions/{id}/git/commit"),
        Some(json!({"message": "again"})),
    );
    assert_eq!(nothing.json()["error"], "nothing to commit");
    let session = json_call(&server, "GET", &format!("/api/sessions/{id}"), None);
    let notices: Vec<&str> = session["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["t"] == "notice")
        .map(|e| e["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        notices,
        [
            "Discarded the changes to readme.txt.",
            format!("Committed {sha}: Add new.txt").as_str()
        ]
    );
    let tree = json_call(&server, "GET", &format!("/api/sessions/{id}/tree"), None);
    let names: Vec<&str> = tree["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["src", ".git", "new.txt", "readme.txt"],
        "folders first, hidden last"
    );
    let file = json_call(
        &server,
        "GET",
        &format!("/api/sessions/{id}/file?path=new.txt"),
        None,
    );
    assert_eq!(file["text"], "a\nb\nc\n");
    assert_eq!(file["rel"], "new.txt");
    fs::write(server.project.join("src/with space.txt"), "spaced").unwrap();
    let spaced = json_call(
        &server,
        "GET",
        &format!("/api/sessions/{id}/file?path=src%2Fwith%20space.txt"),
        None,
    );
    assert_eq!(spaced["text"], "spaced");
    let sub = json_call(
        &server,
        "GET",
        &format!("/api/sessions/{id}/tree?path=src"),
        None,
    );
    assert_eq!(sub["path"], "src");
    server.stop();
}
