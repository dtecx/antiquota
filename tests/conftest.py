import os

import pytest

from agyloop import accounts as a

HERE = os.path.dirname(os.path.abspath(__file__))
ONE, TWO = "one@example.com", "two@example.com"


@pytest.fixture
def home(tmp_path, monkeypatch):
    """A real HOME with an agy setup, and two accounts signed in to agy-loop."""
    h = tmp_path / "home"
    files = {
        ".gemini/GEMINI.md": "rules",
        ".gemini/config/skills/caveman/SKILL.md": "skill",
        ".gemini/config/hooks.json": '{"claude-mem": {}}',
        ".gemini/antigravity-cli/mcp/gitnexus/state": "",
        ".gemini/antigravity-cli/settings.json": '{"permissions": {"allow": ["command(ls)"]}}',
        ".gemini/antigravity-cli/installation_id": "id",
        a.AUTH: "your-own-agy-sign-in",  # as if your agy CLI had used file mode once
        ".gitconfig": "[user]",
        ".gitnexus/registry.json": "{}",
    }
    for p, text in files.items():
        (h / p).parent.mkdir(parents=True, exist_ok=True)
        (h / p).write_text(text)
    monkeypatch.setenv("AGYLOOP_HOME", str(h))
    monkeypatch.setenv("AGYLOOP_DIR", str(h / ".agyloop"))
    monkeypatch.setenv("AGYLOOP_AGY", os.path.join(HERE, "stub_agy.py"))
    for k in ("STUB_429", "STUB_SPENT", "STUB_SAY", "STUB_DONE_AFTER", "STUB_VERDICT", "STUB_SLEEP", "STUB_FLAG_AT",
              "STUB_STALE_ERROR", "STUB_PANEL", "STUB_INELIGIBLE", "STUB_DENY"):
        monkeypatch.delenv(k, raising=False)
    sign_in(ONE, monkeypatch=monkeypatch)
    sign_in(TWO, monkeypatch=monkeypatch)
    return h


def sign_in(email, code="good-code", monkeypatch=None):
    (monkeypatch.setenv if monkeypatch else os.environ.__setitem__)("STUB_EMAIL", email)
    os.environ["STUB_ACCOUNT"] = email + "-token"
    lg = a.Login()
    assert lg.wait_url(10)
    lg.submit(code, timeout=10)
    return lg


@pytest.fixture
def work(tmp_path):
    """A project folder for sessions to work in."""
    w = tmp_path / "project"
    w.mkdir()
    return w
