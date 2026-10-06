import json
import os
import time
from datetime import datetime

import pytest

from agyloop import accounts as a
from agyloop.__main__ import main

HERE = os.path.dirname(os.path.abspath(__file__))


@pytest.fixture
def home(tmp_path, monkeypatch):
    h = tmp_path / "home"
    files = {
        ".gemini/GEMINI.md": "rules",
        ".gemini/config/skills/caveman/SKILL.md": "skill",
        ".gemini/config/hooks.json": '{"claude-mem": {}}',
        ".gemini/antigravity-cli/mcp/gitnexus/state": "",
        ".gemini/antigravity-cli/installation_id": "id",
        a.AUTH: "main-acct",
        ".gitconfig": "[user]",
        ".gitnexus/registry.json": "{}",
    }
    for p, text in files.items():
        (h / p).parent.mkdir(parents=True, exist_ok=True)
        (h / p).write_text(text)
    monkeypatch.setenv("AGYLOOP_HOME", str(h))
    monkeypatch.setenv("AGYLOOP_DIR", str(h / ".agyloop"))
    monkeypatch.setenv("AGYLOOP_AGY", os.path.join(HERE, "stub_agy.py"))
    monkeypatch.setenv("STUB_ACCOUNT", "b-acct")
    for k in ("STUB_429", "STUB_SPENT", "STUB_SAY"):
        monkeypatch.delenv(k, raising=False)
    a.login("b")
    return h


def test_login_shares_stack_and_keeps_auth_private(home):
    s = home / ".agyloop/slots/b"
    assert (s / a.AUTH).read_text() == "b-acct" and not (s / a.AUTH).is_symlink()
    assert (home / a.AUTH).read_text() == "main-acct"
    for p in (".gemini/GEMINI.md", ".gemini/config", ".gemini/antigravity-cli/mcp", ".gitconfig", ".gitnexus"):
        assert os.path.realpath(s / p) == os.path.realpath(home / p), p
    assert not os.path.lexists(s / ".gemini/antigravity-cli/installation_id")
    assert not os.path.lexists(s / ".agyloop")
    # Sign-in ran under an empty HOME: the shared stack is untouched, its output is kept aside.
    assert not (home / ".gemini/config/config.json").exists()
    assert (s / ".agyloop-login/.gemini/config/config.json").exists()
    assert a.unshared(str(s)) == [] and main(["doctor"]) == 0
    # Later changes to the real HOME follow on the next sync.
    (home / ".newtool").write_text("")
    (home / ".gitconfig").unlink()
    a.sync(str(s))
    assert (s / ".newtool").is_symlink() and not os.path.lexists(s / ".gitconfig")


def test_run_is_sticky_and_fails_over(home, capfd, monkeypatch):
    assert main(["run", "-p", "hi"]) == 0
    assert "ran on home" in capfd.readouterr().out
    monkeypatch.setenv("STUB_429", "home")
    assert main(["run", "-p", "hi"]) == a.TEMPFAIL
    assert a.walled_until("main", "gemini") > time.time()
    monkeypatch.delenv("STUB_429")
    capfd.readouterr()
    assert main(["run", "-p", "hi"]) == 0
    assert "ran on b" in capfd.readouterr().out
    # The Claude/GPT basket has its own wall: main still serves it.
    assert main(["run", "--model", "claude-sonnet-4-6", "-p", "hi"]) == 0
    assert "ran on home" in capfd.readouterr().out
    a.set_wall("b", "gemini", time.time() + 60, "test")
    assert main(["run", "-p", "hi"]) == a.TEMPFAIL


def test_busy_account_is_skipped(home, capfd):
    fd = a.try_lease("main")
    try:
        assert main(["run", "-p", "hi"]) == 0
        assert "ran on b" in capfd.readouterr().out
    finally:
        os.close(fd)


def test_only_agy_errors_wall_never_model_text(home, monkeypatch):
    monkeypatch.setenv("STUB_SAY", '{"status": "SUCCESS", "response": "got a 429 quota rate limit"}')
    assert main(["run", "-p", "hi"]) == 0 and not a.walled_until("main", "gemini")
    assert a.is_quota_error(1, "", "Error: RESOURCE_EXHAUSTED")
    assert a.is_quota_error(0, '{"status": "ERROR", "error": "Individual quota reached"}', "")
    assert not a.is_quota_error(0, "", "quota")


def test_probe_walls_spent_basket_until_panel_reset(home, monkeypatch):
    monkeypatch.setenv("STUB_SPENT", "b")
    main(["probe"])
    assert a.walled_until("b", "gemini") == datetime.fromisoformat("2099-01-01T05:00:00Z").timestamp() + 120
    assert not a.walled_until("b", "cg") and not a.walled_until("main", "gemini")
    monkeypatch.delenv("STUB_SPENT")
    main(["probe"])
    assert not a.walled_until("b", "gemini")


def test_parse_real_quota_output():
    with open(os.path.join(HERE, "quota_sample.json")) as f:
        q = a.parse_quota(json.load(f))
    assert set(q) == {"gemini", "cg"} and set(q["gemini"]) == {"5h", "weekly"}
    assert all(0 <= frac <= 1 and reset > 0 for w in q.values() for frac, reset in w.values())
