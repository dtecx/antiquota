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
