from antiquota import policy, stack


def test_policy_sorts_tools_by_risk(home):
    d = lambda mode, tool, args=None: policy.decide(mode, tool, args or {})[0]
    assert d("default", "view_file") == d("plan", "grep_search") == "allow"
    assert d("default", "run_command", {"CommandLine": "ls -la"}) == "allow"           # allow rule command(ls)
    assert d("default", "run_command", {"CommandLine": "ls && rm -rf /"}) == "ask"    # every part must be allowed
    assert d("default", "write_to_file") == "ask"
    assert d("accept-edits", "write_to_file") == "allow"
    assert d("plan", "replace_file_content") == "deny"
    assert d("default", "deploy_firebase") == "ask"
    assert d("bypass", "deploy_firebase") == d("bypass", "run_command", {"CommandLine": "rm x"}) == "allow"
    mcp = {"ServerName": "gitnexus", "ToolName": "query"}
    assert policy.decide("default", "call_mcp_tool", mcp) == ("ask", "", "mcp(gitnexus/query)")
    stack.allow_rule("mcp(gitnexus/*)")
    assert d("default", "call_mcp_tool", mcp) == "allow"
    assert policy.decide("default", "run_command", {"CommandLine": "npm test"})[2] == "command(npm test)"
    assert policy.decide("default", "run_command", {"CommandLine": "pytest -q"})[2] == "command(pytest)"


def test_allow_rules_never_cover_risky_flags(home):
    for r in ("command(find)", "command(git push)", "command(sed)", "command(git reset)", "command(git stash)"):
        stack.allow_rule(r)
    d = lambda cmd: policy.decide("default", "run_command", {"CommandLine": cmd})[0]
    assert d("find . -name '*.go'") == "allow" and d("git push origin main") == "allow"
    for cmd in ("find . -name '*.tmp' -delete", "find . -exec rm {} \;", "git push --force origin main",
                "git push -f origin main", "git push origin :main", "git push origin --delete feature",
                "sed -i '' s/a/b/ x.go", "git reset --hard HEAD~3", "git stash clear", "ls && find / -delete"):
        assert d(cmd) == "ask", cmd


def test_command_parts_respect_quotes_and_catch_hidden_commands(home):
    for r in ("command(ps)", "command(grep)", "command(echo)", "command(ls)", "command(sed -n)"):
        stack.allow_rule(r)
    d = lambda cmd: policy.decide("default", "run_command", {"CommandLine": cmd})[0]
    for cmd in ('ps aux | grep -i -E "agy|bin/olx-mcp" | grep -v grep', "grep -rn 'a;b' . 2>/dev/null",
                "sed -n '1,5p' x.go", "ls >/dev/null 2>&1"):
        assert d(cmd) == "allow", cmd
    for cmd in ("ls\nrm -rf x", "echo $(rm -rf x)", "echo `rm x`", "echo hi > main.go", "ls & rm x",
                "echo 'unbalanced"):
        assert d(cmd) == "ask", cmd
