//! Ports every legacy policy case and adds property checks for shell splitting.
use super::*;
use crate::Mode;
use proptest::prelude::*;
use serde_json::json;

fn rules(allow: &[&str], deny: &[&str]) -> Permissions {
    Permissions {
        allow: allow.iter().map(|r| r.to_string()).collect(),
        deny: deny.iter().map(|r| r.to_string()).collect(),
    }
}
fn verdict(p: &Permissions, mode: Mode, tool: &str, args: serde_json::Value) -> &'static str {
    decide(mode, tool, &args, p).verdict.as_str()
}
fn command(p: &Permissions, line: &str) -> &'static str {
    verdict(
        p,
        Mode::Default,
        "run_command",
        json!({ "CommandLine": line }),
    )
}

#[test]
fn policy_sorts_tools_by_risk() {
    let p = rules(&["command(ls)"], &[]);
    assert_eq!(verdict(&p, Mode::Default, "view_file", json!({})), "allow");
    assert_eq!(verdict(&p, Mode::Plan, "grep_search", json!({})), "allow");
    assert_eq!(command(&p, "ls -la"), "allow");
    assert_eq!(command(&p, "ls && rm -rf /"), "ask");
    assert_eq!(
        verdict(&p, Mode::Default, "write_to_file", json!({})),
        "ask"
    );
    assert_eq!(
        verdict(&p, Mode::AcceptEdits, "write_to_file", json!({})),
        "allow"
    );
    let plan = decide(Mode::Plan, "replace_file_content", &json!({}), &p);
    assert_eq!(plan.verdict, Verdict::Deny);
    assert!(plan.reason.starts_with("Plan mode"));
    assert_eq!(
        verdict(&p, Mode::Default, "deploy_firebase", json!({})),
        "ask"
    );
    assert_eq!(
        verdict(&p, Mode::Bypass, "deploy_firebase", json!({})),
        "allow"
    );
    assert_eq!(
        verdict(
            &p,
            Mode::Bypass,
            "run_command",
            json!({"CommandLine":"rm x"})
        ),
        "allow"
    );
    for tool in [
        "shell_exec",
        "send_command_input",
        "execute_notebook",
        "run_extension_code",
        "restart_dev_server",
    ] {
        assert_eq!(
            verdict(&p, Mode::AcceptEdits, tool, json!({})),
            "ask",
            "{tool}"
        );
    }
    for tool in WRITE_TOOLS {
        assert_eq!(verdict(&p, Mode::AcceptEdits, tool, json!({})), "allow");
        assert_eq!(verdict(&p, Mode::Plan, tool, json!({})), "deny");
    }
}

#[test]
fn mcp_rules_match_exact_server_and_global_and_deny_wins() {
    let call = json!({"ServerName":"gitnexus","ToolName":"query"});
    let none = Permissions::default();
    let d = decide(Mode::Default, "call_mcp_tool", &call, &none);
    assert_eq!(
        (d.verdict, d.rule.as_str()),
        (Verdict::Ask, "mcp(gitnexus/query)")
    );
    let lower = json!({"server":"gitnexus","tool":"query"});
    assert_eq!(
        decide(Mode::Default, "mcp_tool", &lower, &none).rule,
        "mcp(gitnexus/query)"
    );
    let unnamed = decide(
        Mode::Default,
        "call_mcp_tool",
        &json!({"ToolName":"x"}),
        &none,
    );
    assert_eq!((unnamed.verdict, unnamed.rule.as_str()), (Verdict::Ask, ""));
    for allow in ["mcp(gitnexus/query)", "mcp(gitnexus/*)", "mcp(*)"] {
        let p = rules(&[allow], &[]);
        assert_eq!(
            verdict(&p, Mode::Default, "call_mcp_tool", call.clone()),
            "allow"
        );
    }
    for deny in ["mcp(gitnexus/query)", "mcp(gitnexus/*)"] {
        let p = rules(&["mcp(*)"], &[deny]);
        let d = decide(Mode::Default, "call_mcp_tool", &call, &p);
        assert_eq!(d.verdict, Verdict::Deny);
        assert!(d.reason.contains("MCP tool"));
    }
}

#[test]
fn command_rules_match_whole_words_and_suggest_rules() {
    let p = rules(&["command(git status)", "command(npm)"], &[]);
    assert_eq!(command(&p, "git status"), "allow");
    assert_eq!(command(&p, "git status --short"), "allow");
    assert_eq!(command(&p, "git statusx"), "ask");
    assert_eq!(command(&p, "npm test"), "allow");
    let ask = |line: &str| {
        decide(
            Mode::Default,
            "run_command",
            &json!({"CommandLine": line}),
            &p,
        )
    };
    assert_eq!(ask("npm-check").rule, "command(npm-check)");
    assert_eq!(ask("cargo build --release").rule, "command(cargo build)");
    assert_eq!(ask("cargo --version").rule, "command(cargo)");
    assert_eq!(ask("pytest -q").rule, "command(pytest)");
    assert_eq!(
        decide(
            Mode::Default,
            "run_command",
            &json!({"CommandLine":"npm test"}),
            &Permissions::default()
        )
        .rule,
        "command(npm test)"
    );
    let star = rules(&["command(*)"], &[]);
    assert_eq!(command(&star, "anything at all"), "allow");
    assert_eq!(command(&star, "find . -delete"), "ask");
}

#[test]
fn git_commit_uses_the_git_commit_command() {
    let p = rules(&["command(git commit)"], &[]);
    assert_eq!(verdict(&p, Mode::Default, "git_commit", json!({})), "allow");
    assert_eq!(
        verdict(
            &p,
            Mode::Default,
            "git_commit",
            json!({"CommandLine":"rm x"})
        ),
        "ask"
    );
}

#[test]
fn denies_win_even_when_the_line_cannot_be_parsed() {
    let p = rules(&["command(ls)", "command(rm)"], &["command(rm)"]);
    let d = decide(
        Mode::Default,
        "run_command",
        &json!({"CommandLine":"ls; rm -rf x"}),
        &p,
    );
    assert_eq!(d.verdict, Verdict::Deny);
    assert!(d.reason.contains("deny rule"));
    assert_eq!(command(&p, "ls && rm 'unbalanced"), "deny");
    assert_eq!(command(&p, "rm"), "deny");
}

#[test]
fn allow_rules_never_cover_risky_flags() {
    let p = rules(
        &[
            "command(find)",
            "command(git push)",
            "command(sed)",
            "command(git reset)",
            "command(git stash)",
            "command(git clean)",
            "command(git checkout)",
            "command(git branch)",
            "command(ls)",
        ],
        &[],
    );
    assert_eq!(command(&p, "find . -name '*.go'"), "allow");
    assert_eq!(command(&p, "git push origin main"), "allow");
    assert_eq!(command(&p, "git branch -d merged"), "allow");
    for line in [
        "find . -name '*.tmp' -delete",
        "find . -exec rm {} \\;",
        "find . -execdir rm {} +",
        "find . -ok rm {} \\;",
        "find . -okdir rm {} \\;",
        "find . -fprint out.txt",
        "find . -fprintf out.txt %p",
        "git push --force origin main",
        "git push --force-with-lease origin main",
        "git push -f origin main",
        "git push origin :main",
        "git push origin --delete feature",
        "git push --mirror backup",
        "git push -d origin feature",
        "sed -i '' s/a/b/ x.go",
        "git reset --hard HEAD~3",
        "git clean -fd",
        "git clean -xdf",
        "git clean --force",
        "git branch -f main HEAD~1",
        "git checkout -- main.go",
        "git branch -D feature",
        "git stash drop",
        "git stash clear",
        "ls && find / -delete",
    ] {
        assert_eq!(command(&p, line), "ask", "{line}");
    }
}

#[test]
fn command_parts_respect_quotes_and_catch_hidden_commands() {
    let p = rules(
        &[
            "command(ps)",
            "command(grep)",
            "command(echo)",
            "command(ls)",
            "command(sed -n)",
        ],
        &[],
    );
    for line in [
        "ps aux | grep -i -E \"agy|bin/olx-mcp\" | grep -v grep",
        "grep -rn 'a;b' . 2>/dev/null",
        "sed -n '1,5p' x.go",
        "ls >/dev/null 2>&1",
        "ls &&\necho done",
        "ls 2>&-",
        "grep x < input.txt",
    ] {
        assert_eq!(command(&p, line), "allow", "{line:?}");
    }
    for line in [
        "ls\nrm -rf x",
        "ls\n\nrm -rf x",
        "echo $(rm -rf x)",
        "echo `rm x`",
        "echo hi > main.go",
        "echo hi >> main.go",
        "ls >& out.txt",
        "ls <> out.txt",
        "ls & rm x",
        "echo 'unbalanced",
        "echo \"unbalanced",
        "echo trailing\\",
        "diff <(ls) <(ls)",
        "",
        "   ",
    ] {
        assert_eq!(command(&p, line), "ask", "{line:?}");
    }
}

#[test]
fn shell_splitting_keeps_quoted_operators_and_removes_quotes() {
    use shell::command_parts;
    assert_eq!(
        command_parts(r#"grep -E "a|b" x | wc -l"#).unwrap(),
        ["grep -E a|b x", "wc -l"]
    );
    assert_eq!(
        command_parts("echo 'a;b' && ls").unwrap(),
        ["echo a;b", "ls"]
    );
    assert_eq!(
        command_parts(r#"echo "say \"hi\" \n""#).unwrap(),
        [r#"echo say "hi" \n"#]
    );
    assert_eq!(
        command_parts("a || b |& c ;; d").unwrap(),
        ["a", "b", "c", "d"]
    );
    assert_eq!(command_parts("a\\ b").unwrap(), ["a b"]);
    assert!(command_parts("echo $(x)").is_none());
}

#[test]
fn permanent_rules_never_allow_arbitrary_programs() {
    for rule in [
        "command(bash)",
        "command(sh -c)",
        "command(env)",
        "command(xargs)",
        "command(awk)",
        "command(curl)",
        "command(sudo)",
        "command(python3)",
        "command(python3 -c)",
        "command(node -e)",
        "command(npx)",
        "command(*)",
        "command()",
    ] {
        assert!(permanent_rule_error(rule).is_some(), "{rule}");
    }
    for rule in [
        "command(ls)",
        "command(git push)",
        "command(npx vitest)",
        "command(python3 manage.py)",
        "command(go run)",
        "mcp(*)",
        "mcp(gitnexus/query)",
    ] {
        assert!(permanent_rule_error(rule).is_none(), "{rule}");
    }
    let p = Permissions::default();
    let ask = |line: &str| {
        decide(
            Mode::Default,
            "run_command",
            &json!({"CommandLine": line}),
            &p,
        )
    };
    assert_eq!(ask("bash -c 'rm -rf x'").rule, "");
    assert_eq!(ask("xargs rm").rule, "");
    assert_eq!(ask("python3 -c 'print(1)'").rule, "");
    assert_eq!(
        ask("python3 manage.py test").rule,
        "command(python3 manage.py)"
    );
}

#[test]
fn rule_lists_are_validated_and_deduplicated() {
    assert_eq!(
        clean_rules(
            "allow",
            &json!(["command(ls)", "command(ls)", "command(git status)"])
        )
        .unwrap(),
        ["command(ls)", "command(git status)"]
    );
    assert_eq!(
        clean_rules("allow", &json!([42])).unwrap_err(),
        "allow must be a list of rule strings"
    );
    assert!(clean_rules("deny", &json!("command(rm)")).is_err());
    assert!(clean_rules("deny", &json!([""])).is_err());
    assert!(clean_rules("deny", &json!(["x".repeat(300)])).is_err());
    assert!(clean_rules("deny", &json!(["x".repeat(299)])).is_ok());
    assert!(clean_rules("allow", &json!(["command(bash)"])).is_err());
    assert_eq!(
        clean_rules("deny", &json!(["command(bash)"])).unwrap(),
        ["command(bash)"]
    );
}

proptest! {
    #[test]
    fn risky_flags_ask_after_any_allowed_prefix(prefix in "[a-z]{1,8}( [a-z0-9]{1,6}){0,2}") {
        let p = rules(&["command(*)"], &[]);
        for risky in ["find . -delete", "git push --force", "sed -i x", "git reset --hard"] {
            let line = format!("{prefix} && {risky}");
            prop_assert_eq!(command(&p, &line), "ask");
        }
    }

    #[test]
    fn quoted_separators_never_create_commands(word in "[a-z]{1,8}", sep in prop::sample::select(vec![";", "|", "&&", "||", "&", "\n"])) {
        let line = format!("echo '{word}{sep}{word}'");
        prop_assert_eq!(shell::command_parts(&line).map(|p| p.len()), Some(1));
        let line = format!("echo \"{word}{sep}{word}\"");
        prop_assert_eq!(shell::command_parts(&line).map(|p| p.len()), Some(1));
    }

    #[test]
    fn arbitrary_lines_never_panic_and_never_allow_unlisted_commands(line in "\\PC{0,80}") {
        let p = rules(&["command(ls)"], &[]);
        let d = command(&p, &line);
        if d == "allow" {
            let parts = shell::command_parts(&line).unwrap();
            prop_assert!(parts.iter().all(|part| part == "ls" || part.starts_with("ls ")));
        }
    }
}
