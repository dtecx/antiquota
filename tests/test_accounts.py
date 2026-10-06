import json
import os
import time
from datetime import datetime

from agyloop import accounts as a
from agyloop.__main__ import main

from conftest import HERE, ONE, TWO, sign_in


def test_accounts_are_named_by_email_and_use_their_own_stack(home):
    assert a.slots() == [ONE, TWO]
    s, stack = home / ".agyloop/slots" / ONE, home / ".agyloop/stack"
    assert (s / a.AUTH).read_text() == ONE + "-token" and not (s / a.AUTH).is_symlink()
    assert (home / a.AUTH).read_text() == "your-own-agy-sign-in"
    # .gemini comes from agy-loop's stack, everything else from the real HOME.
    for p in (".gemini/GEMINI.md", ".gemini/config", ".gemini/antigravity-cli/mcp",
              ".gemini/antigravity-cli/settings.json", ".gemini/antigravity-cli/conversations"):
        assert os.path.realpath(s / p) == os.path.realpath(stack / p), p
    for p in (".gitconfig", ".gitnexus"):
        assert os.path.realpath(s / p) == os.path.realpath(home / p), p
    assert (s / ".gemini/config/skills/caveman/SKILL.md").read_text() == "skill"
    assert not os.path.lexists(s / ".gemini/antigravity-cli/installation_id")
    assert not os.path.lexists(s / ".agyloop")
    # Sign-in never touched either stack; what it created is kept aside.
    assert not (home / ".gemini/config/config.json").exists() and not (stack / ".gemini/config/config.json").exists()
    assert (s / ".agyloop-login/.gemini/config/config.json").exists()
    assert a.unshared(str(s)) == [] and main(["doctor"]) == 0


def test_stack_is_separate_from_your_agy_cli(home):
    stack_settings = home / ".agyloop/stack/.gemini/antigravity-cli/settings.json"
    stack_settings.write_text('{"permissions": {"allow": ["command(rm)"]}}')
    (home / ".gemini/config/skills/new-in-cli").mkdir()
    assert "command(ls)" in (home / ".gemini/antigravity-cli/settings.json").read_text()
    assert not (home / ".agyloop/slots" / ONE / ".gemini/config/skills/new-in-cli").exists()
    (home / ".agyloop/stack/.gemini/antigravity-cli/conversations/c1").write_text("x")
    a.seed_stack(force=True)  # re-copy config on purpose: settings reset, conversations kept
    assert "command(ls)" in stack_settings.read_text()
    assert (home / ".agyloop/slots" / ONE / ".gemini/config/skills/new-in-cli").exists()
    assert (home / ".agyloop/stack/.gemini/antigravity-cli/conversations/c1").exists()
    # New real-HOME entries follow on the next sync.
    (home / ".newtool").write_text("")
    a.sync(str(home / ".agyloop/slots" / ONE))
    assert (home / ".agyloop/slots" / ONE / ".newtool").is_symlink()


def test_sign_in_needs_the_right_code_and_never_half_creates_an_account(home):
    lg = sign_in("three@example.com", code="wrong")
    assert lg.state == "failed" and "authentication failed" in lg.error
    assert a.slots() == [ONE, TWO]
    lg = sign_in("three@example.com")
    assert lg.state == "done" and lg.name == "three@example.com" and a.slots() == [ONE, "three@example.com", TWO]
    a.remove("three@example.com")
    assert a.slots() == [ONE, TWO]


def test_run_is_sticky_and_fails_over(home, capfd, monkeypatch):
    assert main(["run", "-p", "hi"]) == 0
    assert f"ran on {ONE}" in capfd.readouterr().out
    monkeypatch.setenv("STUB_429", ONE)
    assert main(["run", "-p", "hi"]) == a.TEMPFAIL
    assert a.walled_until(ONE, "gemini") > time.time()
    monkeypatch.delenv("STUB_429")
    capfd.readouterr()
    assert main(["run", "-p", "hi"]) == 0
    assert f"ran on {TWO}" in capfd.readouterr().out
    # The Claude/GPT basket has its own wall: the first account still serves it.
    assert main(["run", "--model", "claude-sonnet-4-6", "-p", "hi"]) == 0
    assert f"ran on {ONE}" in capfd.readouterr().out
    a.set_wall(TWO, "gemini", time.time() + 60, "test")
    assert main(["run", "-p", "hi"]) == a.TEMPFAIL


def test_busy_account_is_skipped(home, capfd):
    fd = a.try_lease(ONE)
    try:
        assert main(["run", "-p", "hi"]) == 0
        assert f"ran on {TWO}" in capfd.readouterr().out
    finally:
        os.close(fd)


def test_only_agy_errors_wall_never_model_text(home, monkeypatch):
    monkeypatch.setenv("STUB_SAY", '{"status": "SUCCESS", "response": "got a 429 quota rate limit"}')
    assert main(["run", "-p", "hi"]) == 0 and not a.walled_until(ONE, "gemini")
    assert a.is_quota_error(1, "", "Error: RESOURCE_EXHAUSTED")
    assert a.is_quota_error(0, '{"status": "ERROR", "error": "Individual quota reached"}', "")
    assert not a.is_quota_error(0, "", "quota")


def test_probe_walls_spent_basket_until_panel_reset(home, monkeypatch):
    monkeypatch.setenv("STUB_SPENT", TWO)
    main(["probe"])
    assert a.walled_until(TWO, "gemini") == datetime.fromisoformat("2099-01-01T05:00:00Z").timestamp() + 120
    assert not a.walled_until(TWO, "cg") and not a.walled_until(ONE, "gemini")
    monkeypatch.delenv("STUB_SPENT")
    main(["probe"])
    assert not a.walled_until(TWO, "gemini")


def test_parse_real_quota_output():
    with open(os.path.join(HERE, "quota_sample.json")) as f:
        q = a.parse_quota(json.load(f))
    assert set(q) == {"gemini", "cg"} and set(q["gemini"]) == {"5h", "weekly"}
    assert all(0 <= frac <= 1 and reset > 0 for w in q.values() for frac, reset in w.values())


def test_every_agy_call_keeps_the_token_in_the_account_folder(home):
    cmd, env = a.agy_cmd(str(home / ".agyloop/slots" / ONE), ["-p", "hi"])
    assert env["HOME"] == env["USERPROFILE"] == str(home / ".agyloop/slots" / ONE)
    assert env["SSH_CONNECTION"]  # agy: "Using file-based token storage because SSH session detected"
    assert a.whoami(ONE) == ONE and a.whoami(TWO) == TWO


def test_parse_quota_when_weekly_is_spent_and_5h_disabled(home):
    with open(os.path.join(HERE, "quota_sample_spent.json")) as f:
        q = a.parse_quota(json.load(f))
    weekly_reset = q["gemini"]["weekly"][1]
    assert q["gemini"]["weekly"][0] == 0 and q["gemini"]["5h"] == (0.0, None)
    os.environ["STUB_PANEL"] = os.path.join(HERE, "quota_sample_spent.json")
    try:
        assert a.probe_slot(ONE) is not None
    finally:
        del os.environ["STUB_PANEL"]
    assert a.walled_until(ONE, "gemini") == weekly_reset + 120  # walled until the weekly reset
    assert not a.walled_until(ONE, "cg")


def test_an_account_google_wants_verified_is_kept_and_parked(home, monkeypatch):
    monkeypatch.setenv("STUB_INELIGIBLE", "new@example.com")
    lg = sign_in("new@example.com", monkeypatch=monkeypatch)
    assert lg.state == "verify" and lg.name == "new@example.com"
    assert lg.verify_url.startswith("https://accounts.google.com/signin/continue")
    assert "new@example.com" in a.slots() and a.needs_verify("new@example.com") == lg.verify_url
    assert a.walled_until("new@example.com", "gemini") and a.walled_until("new@example.com", "cg")
    assert a.probe_slot("new@example.com") is None  # still not verified: stays parked
    monkeypatch.delenv("STUB_INELIGIBLE")            # the user verified it with Google
    assert a.probe_slot("new@example.com") is not None
    assert not a.needs_verify("new@example.com") and not a.walled_until("new@example.com", "gemini")
