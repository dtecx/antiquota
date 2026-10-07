//! Proves CLI sign-in, doctor metadata and reseeding with synthetic accounts only.
use crate::{
    setup,
    support::{Server, bounded_output},
};
use serde_json::json;
use std::{fs, process::Stdio};

#[test]
fn cli_login_adopts_email_and_rejects_bad_codes_without_partial_slots() {
    let server = Server::new_stopped("cli-login", true);
    setup::script(&server, json!({"login":{"email":"three@example.com"}}));
    let code = server.root.join("code.txt");
    setup::write(&code, "wrong-code\n");
    let output = bounded_output(
        server
            .command()
            .arg("login")
            .stdin(Stdio::from(fs::File::open(&code).unwrap())),
    );
    let rejected = json!({"exit":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)});
    assert_eq!(output.status.code(), Some(1));
    assert!(!server.data.join("slots/three@example.com").exists());
    setup::write(&code, "good-code\n");
    let output = bounded_output(
        server
            .command()
            .arg("login")
            .stdin(Stdio::from(fs::File::open(&code).unwrap())),
    );
    let mut slots: Vec<_> = fs::read_dir(server.data.join("slots"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    slots.sort();
    setup::evidence(
        "cli-login-output",
        json!({"rejected":rejected,"accepted":{"exit":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)},"adoptedSlots":slots}),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).ends_with(&format!(
            "three@example.com signed in; it uses antiquota's stack in {}\n",
            server.data.join("stack").display()
        )),
        "stdout: {}\nstderr: {}\nslots: {slots:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let token = server
        .data
        .join("slots/three@example.com/.gemini/antigravity-cli/antigravity-oauth-token");
    assert!(token.is_file() && !token.is_symlink());
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
        "cli-login",
        json!({"badCodeExit":1,"successfulIdentity":"three@example.com","privateToken":true,"temporaryHomesRemoved":true,"browserCalls":0}),
    );
}

#[test]
fn doctor_reports_loaded_metadata_and_identity_mismatch_without_changing_tokens() {
    let server = Server::new_stopped("cli-doctor", true);
    setup::captured_home(&server);
    let token = server
        .data
        .join("slots/two@example.com/.gemini/antigravity-cli/antigravity-oauth-token");
    let before = fs::read(&token).unwrap();
    setup::script(
        &server,
        json!({"metadata":{"skills":["example"]},"metadata_by_slot":{setup::TWO:{"skills":["example"],"identity":"wrong@example.com"}}}),
    );
    let output = bounded_output(server.command().arg("doctor"));
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("one@example.com: ok: signs in as itself"));
    assert!(text.contains("two@example.com: PROBLEM: signs in as wrong@example.com"));
    assert_eq!(fs::read(&token).unwrap(), before);
    setup::script(&server, json!({"metadata":{"skills":["example"]}}));
    let output = bounded_output(server.command().arg("doctor"));
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("skills (1): example"));
    assert!(text.contains("hooks:   antiquota"));
    let records = setup::calls(&server);
    for slot in [setup::ONE, setup::TWO] {
        assert!(
            records
                .iter()
                .any(|row| row["slot"] == slot && setup::argument(row, "-p") == Some("/skills"))
        );
        assert!(
            records
                .iter()
                .any(|row| row["slot"] == slot && setup::argument(row, "-p") == Some("/hooks"))
        );
        assert!(
            records
                .iter()
                .any(|row| row["slot"] == slot && setup::argument(row, "--log-file").is_some())
        );
    }
    assert_eq!(fs::read(token).unwrap(), before);
    setup::evidence(
        "cli-doctor",
        json!({"mismatchExit":1,"healthyExit":0,"identityLogUsed":true,"loadedSkillsAndHooksUsed":true,"tokenUnchanged":true}),
    );
}

#[test]
fn cli_stack_reseed_replaces_configuration_and_preserves_shared_conversations() {
    let server = Server::new_stopped("cli-reseed", true);
    setup::captured_home(&server);
    let conversation = server
        .data
        .join("stack/.gemini/antigravity-cli/conversations/fixture.json");
    setup::write(&conversation, "private synthetic conversation\n");
    setup::write(
        &server.data.join("stack/.gemini/GEMINI.md"),
        "Old own configuration.\n",
    );
    setup::write(
        &server.home.join(".gemini/GEMINI.md"),
        "Replacement source configuration.\n",
    );
    let output = bounded_output(server.command().args(["stack", "reseed"]));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("config re-copied from ~/.gemini (conversations kept)")
    );
    assert_eq!(
        fs::read_to_string(server.data.join("stack/.gemini/GEMINI.md")).unwrap(),
        "Replacement source configuration.\n"
    );
    assert_eq!(
        fs::read_to_string(conversation).unwrap(),
        "private synthetic conversation\n"
    );
    setup::evidence(
        "cli-reseed",
        json!({"sourceConfigurationCopied":true,"sharedConversationPreserved":true}),
    );
}
