"""Which tool calls agy may make on its own, and which wait for the user.

agy in headless mode cannot ask anyone and ignores allow-rules, so agy-loop runs it with
--dangerously-skip-permissions while its own PreToolUse hook is on, and this module is the
permission layer instead. Decisions:

  allow  reading, searching, agy's own bookkeeping; anything in bypass mode
  rules  shell commands (`command(...)`) and MCP tools (`mcp(server/tool)`) in agy-loop's
         allow/deny lists; otherwise the user is asked
  mode   file edits: accept-edits allows them, plan mode refuses them, default asks
  ask    deploys, databases, package installs and other side effects outside the project
"""
from . import stack

COMMAND_TOOLS = {"run_command", "shell_exec", "send_command_input", "execute_notebook", "run_extension_code",
                 "restart_dev_server", "git_commit"}
WRITE_TOOLS = {"write_to_file", "replace_file_content", "multi_replace_file_content", "sed_file", "file_change",
               "edit_notebook", "move", "delete_directory", "write_blob"}
MCP_TOOLS = {"call_mcp_tool", "mcp_tool"}
ASK_TOOLS = {"deploy_firebase", "set_up_firebase", "cloud_sql_execute_sql", "cloud_sql_update_schema",
             "set_up_cloud_sql", "install_applet_package", "install_applet_dependencies", "post_pr_review"}


def _mcp_rule(args):
    server, tool = args.get("ServerName") or args.get("server") or "", args.get("ToolName") or args.get("tool") or ""
    return f"mcp({server}/{tool})" if server else ""


def _mcp_verdict(rule):
    allow, deny = stack.permissions()
    server = rule[4:].split("/", 1)[0]
    wide = f"mcp({server}/*)"
    if rule in deny or wide in deny:
        return "deny"
    if rule in allow or wide in allow or "mcp(*)" in allow:
        return "allow"
    return None


def decide(mode, tool, args):
    """(decision, reason, rule): decision is "allow", "deny" or "ask" (the user decides);
    rule is what "Always allow" would add."""
    args = args or {}
    if mode == "bypass":
        return "allow", "", ""
    if tool == "run_command" or (tool == "git_commit"):
        cmd = str(args.get("CommandLine") or ("git commit" if tool == "git_commit" else ""))
        verdict = stack.command_verdict(cmd)
        if verdict == "deny":
            return "deny", "A deny rule in agy-loop's permissions blocks this command.", ""
        if verdict == "allow":
            return "allow", "", ""
        return "ask", "", stack.suggest_rule(cmd)
    if tool in COMMAND_TOOLS:
        return "ask", "", ""
    if tool in WRITE_TOOLS:
        if mode == "accept-edits":
            return "allow", "", ""
        if mode == "plan":
            return "deny", "Plan mode: describe the change instead of making it.", ""
        return "ask", "", ""
    if tool in MCP_TOOLS:
        rule = _mcp_rule(args)
        verdict = _mcp_verdict(rule) if rule else None
        if verdict == "deny":
            return "deny", "A deny rule in agy-loop's permissions blocks this MCP tool.", ""
        if verdict == "allow":
            return "allow", "", ""
        return "ask", "", rule
    if tool in ASK_TOOLS:
        return "ask", "", ""
    return "allow", "", ""
