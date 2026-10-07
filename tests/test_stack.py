import json

import pytest

from antiquota import stack


def put(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


@pytest.fixture
def setups(home):
    """Your agy CLI and IDE setups, with things antiquota does not have yet."""
    cli, ide = home / ".gemini", home / ".gemini/antigravity"
    hook = str(cli / "config/hooks/gitnexus-agy/hook.cjs")
    put(cli / "config/hooks.json", json.dumps({"claude-mem": {}, "gitnexus": {"PostToolUse": [
        {"matcher": "*", "hooks": [{"type": "command", "command": "node " + hook}]}]}}))
    put(cli / "config/hooks/gitnexus-agy/hook.cjs", "// bridge")
    put(cli / "config/mcp_config.json", json.dumps({"mcpServers": {"gitnexus": {"command": "gitnexus", "args": ["mcp"]}}}))
    put(cli / "config/rules/caveman.md", "talk short")
    put(ide / "skills/ide-only/SKILL.md", "---\ndescription: from the IDE\n---\n")
    put(ide / "mcp_config.json", json.dumps({"mcpServers": {"ide-mcp": {"url": "https://x.example/mcp"}}}))
    return home


def rows(kind):
    return {r["name"]: r for r in stack.overview()[kind]}


def test_overview_compares_with_your_cli_and_ide(setups):
    skills = rows("skills")
    assert skills["caveman"]["here"] and skills["caveman"]["cli"] == "same"
    assert not skills["ide-only"]["here"] and skills["ide-only"]["ide"] == "differs"
    assert skills["ide-only"]["about"] == "from the IDE"
    assert rows("hooks")["gitnexus"]["cli"] == "differs" and not rows("hooks")["gitnexus"]["here"]
    assert rows("mcps")["ide-mcp"]["ide"] == "differs"


def test_copy_items_in_and_rewrite_paths_into_the_stack(setups):
    stack.copy("cli", "hook", "gitnexus")
    hooks = json.loads((setups / ".antiquota/stack/.gemini/config/hooks.json").read_text())
    cmd = hooks["gitnexus"]["PostToolUse"][0]["hooks"][0]["command"]
    assert str(setups / ".antiquota/stack/.gemini/config/hooks/gitnexus-agy/hook.cjs") in cmd
    assert (setups / ".antiquota/stack/.gemini/config/hooks/gitnexus-agy/hook.cjs").exists()
    assert rows("hooks")["gitnexus"]["cli"] == "same"
    stack.copy("ide", "skill", "ide-only")
    stack.copy("ide", "mcp")
    stack.copy("cli", "rule", "caveman.md")
    assert rows("skills")["ide-only"]["ide"] == "same" and rows("mcps")["ide-mcp"]["here"]
    with pytest.raises(ValueError):
        stack.copy("ide", "hook", "x")  # the IDE has no hooks.json


def test_edit_and_delete_touch_only_the_stack(setups):
    cli_md = (setups / ".gemini/GEMINI.md").read_text()
    stack.write_file("GEMINI.md", "antiquota rules")
    assert (setups / ".gemini/GEMINI.md").read_text() == cli_md
    assert stack.overview()["gemini_md"]["cli"] == "differs"
    stack.copy("cli", "gemini_md")
    assert stack.read_file("GEMINI.md")["text"] == cli_md
    stack.write_file("config/skills/new/SKILL.md", "---\ndescription: mine\n---\n")
    assert rows("skills")["new"]["here"] and rows("skills")["new"]["cli"] is None
    stack.delete("skill", "new")
    assert "new" not in rows("skills")
    stack.put_item("mcp", "local", {"command": "srv"})
    assert stack.get_item("mcp", "local") == {"command": "srv"}
    stack.set_enabled("hook", "claude-mem", False)
    assert rows("hooks")["claude-mem"]["enabled"] is False
    with pytest.raises(ValueError):
        stack.write_file("config/mcp_config.json", "{broken")
    for bad in ("../../.gemini/GEMINI.md", "antigravity-cli/conversations/x", "/etc/passwd"):
        with pytest.raises(ValueError):
            stack.write_file(bad, "x")
    assert (setups / ".gemini/antigravity").exists() and "skills" in [p.name for p in (setups / ".gemini/antigravity").iterdir()]


def test_gui_defaults_round_trip(home):
    assert stack.defaults()["model"] == "gemini-3.8-flash-high"
    assert stack.set_defaults({"model": "claude-sonnet-4-6", "bogus": 1})["model"] == "claude-sonnet-4-6"
    assert "bogus" not in stack.defaults()
