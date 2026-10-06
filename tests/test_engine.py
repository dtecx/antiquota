import json
import threading
import time

from agyloop import accounts as a
from agyloop.engine import CONTINUE_AFTER_QUOTA, Engine, parse_verdict

from conftest import ONE, TWO


def finish(eng, sid, timeout=20):
    s = eng.sessions[sid]
    deadline = time.time() + timeout
    while s.busy and time.time() < deadline:
        time.sleep(0.05)
    assert not s.busy, "session still running"


def calls(work):
    return [json.loads(line) for line in (work / ".stub_calls.jsonl").read_text().splitlines()]


def kinds(eng, sid, t="notice"):
    return [e.get("kind") for e in eng.events(sid) if e["t"] == t]


def test_turn_streams_persists_and_survives_restart(home, work):
    eng = Engine()
    q = eng.subscribe()
    sid = eng.create(str(work), "gemini-3.8-flash-low")["id"]
    eng.send(sid, "hello")
    finish(eng, sid)
    live = []
    while not q.empty():
        live.append(q.get())
    assert "delta" in [e["t"] for e in live]
    evs = eng.events(sid)
    assert [e["t"] for e in evs] == ["user", "turn_start", "tool", "assistant", "turn_end"]
    tool = evs[2]
    assert tool["name"] == "write_to_file" and "+turn 1" in tool["diff"]
    assert evs[3]["text"].startswith(f"turn 1 on {ONE}")
    meta = eng.sessions[sid].meta
    assert meta["title"] == "hello" and meta["conversation_id"].startswith(f"conv-{ONE}")
    assert meta["usage"] == {"input": 100, "output": 10} and meta["status"] == "idle"
    assert calls(work)[0]["args"][calls(work)[0]["args"].index("--model") + 1] == "gemini-3.8-flash-low"
    again = Engine()
    assert [e["t"] for e in again.events(sid)] == [e["t"] for e in evs]
    assert again.sessions[sid].meta["conversation_id"] == meta["conversation_id"]


def test_quota_wall_passes_the_turn_on_in_the_same_conversation(home, work, monkeypatch):
    eng = Engine()
    sid = eng.create(str(work), "gemini-3.8-flash-low")["id"]
    eng.send(sid, "first")
    finish(eng, sid)
    conv = eng.sessions[sid].meta["conversation_id"]
    monkeypatch.setenv("STUB_429", ONE)
    eng.send(sid, "second")
    finish(eng, sid)
    last = calls(work)[-1]
    assert last["slot"] == TWO
    assert last["args"][1] == CONTINUE_AFTER_QUOTA
    assert last["args"][last["args"].index("--conversation") + 1] == conv
    assert "handoff" in kinds(eng, sid) and a.walled_until(ONE, "gemini")
    assert [e["status"] for e in eng.events(sid) if e["t"] == "turn_end"] == ["SUCCESS", "quota", "SUCCESS"]


def test_every_account_walled_reports_instead_of_hanging(home, work):
    for s in (ONE, TWO):
        a.set_wall(s, "gemini", time.time() + 3600, "test")
    eng = Engine()
    sid = eng.create(str(work), "gemini-3.8-flash-low")["id"]
    eng.send(sid, "hi")
    finish(eng, sid)
    assert kinds(eng, sid) == ["wall"]
    # The Claude/GPT pool is separate and still works.
    eng.update(sid, {"model": "claude-sonnet-4-6"})
    eng.send(sid, "hi")
    finish(eng, sid)
    assert calls(work)[-1]["slot"] == ONE


def test_goal_pushes_on_until_claimed_and_verified(home, work, monkeypatch):
    monkeypatch.setenv("STUB_DONE_AFTER", "2")
    eng = Engine()
    sid = eng.create(str(work), "gemini-3.8-flash-low", mode="accept-edits")["id"]
    eng.set_goal(sid, "write notes", check="test -f notes.txt", verifier=True, verifier_model="claude-sonnet-4-6")
    finish(eng, sid)
    g = eng.sessions[sid].meta["goal"]
    assert g["state"] == "met" and g["round"] == 2
    users = [e["kind"] for e in eng.events(sid) if e["t"] == "user"]
    assert users == ["goal", "nudge"]
    assert kinds(eng, sid) == ["goal", "goal", "check", "verdict", "goal"]
    verifier = [c for c in calls(work) if "VERDICT:" in c["args"][1]]
    assert len(verifier) == 1 and "claude-sonnet-4-6" in verifier[0]["args"]


def test_goal_feeds_failed_checks_and_verdicts_back(home, work, monkeypatch):
    monkeypatch.setenv("STUB_FLAG_AT", "2")
    monkeypatch.setenv("STUB_VERDICT", "no")
    eng = Engine()
    sid = eng.create(str(work), "gemini-3.8-flash-low")["id"]
    eng.set_goal(sid, "make flag", check="test -f flag", verifier=True, max_rounds=3)
    finish(eng, sid)
    nudges = [e["text"] for e in eng.events(sid) if e["t"] == "user" and e["kind"] == "nudge"]
    assert "`test -f flag` exited 1" in nudges[0]
    assert "tests are missing" in nudges[1]
    assert eng.sessions[sid].meta["goal"]["state"] == "failed"


def test_stop_ends_the_turn_and_pauses_the_goal(home, work, monkeypatch):
    monkeypatch.setenv("STUB_SLEEP", "10")
    eng = Engine()
    sid = eng.create(str(work), "gemini-3.8-flash-low")["id"]
    eng.set_goal(sid, "long job", verifier=False)
    time.sleep(1)
    t0 = time.time()
    eng.stop(sid)
    finish(eng, sid, timeout=5)
    assert time.time() - t0 < 5
    assert eng.sessions[sid].meta["goal"]["state"] == "paused"
    assert [e["status"] for e in eng.events(sid) if e["t"] == "turn_end"] == ["stopped"]


def test_messages_sent_while_busy_are_queued(home, work, monkeypatch):
    monkeypatch.setenv("STUB_SLEEP", "1")
    eng = Engine()
    sid = eng.create(str(work), "gemini-3.8-flash-low")["id"]
    assert eng.send(sid, "one") == "started"
    assert eng.send(sid, "two") == "queued"
    finish(eng, sid)
    evs = [(e["t"], str(e.get("text", e.get("status", e.get("kind", ""))))[:50]) for e in eng.events(sid)]
    assert [t for k, t in evs if k == "user"] == ["one", "two"], evs
    assert threading.active_count() >= 1


def test_verdict_line_is_parsed_from_free_text():
    assert parse_verdict("ok\n\n**VERDICT: DONE**")["done"] is True
    v = parse_verdict("- no tests\n* README missing\nVERDICT: NOT_DONE\n")
    assert v["done"] is False and v["missing"] == ["no tests", "README missing"]
    assert parse_verdict("I think it is done.") is None
