import json
import threading
import time
import urllib.error
import urllib.request

import pytest

from antiquota import server

from conftest import ONE, TWO


@pytest.fixture
def gui(home):
    srv = server.make_server(0)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    yield srv, server.Handler.app
    srv.shutdown()


def call(srv, method, path, body=None, token=None, host=None):
    port = srv.server_address[1]
    req = urllib.request.Request(f"http://127.0.0.1:{port}{path}", method=method,
                                 data=json.dumps(body).encode() if body is not None else None)
    if token:
        req.add_header("X-Antiquota-Token", token)
    if body is not None:
        req.add_header("Content-Type", "application/json")
    if host:
        req.add_header("Host", host)
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def test_page_carries_token_and_api_demands_it(gui):
    srv, app = gui
    code, page = call(srv, "GET", "/")
    assert code == 200 and app.token.encode() in page
    assert call(srv, "GET", "/api/state")[0] == 403
    assert call(srv, "GET", "/api/state", token="wrong")[0] == 403
    assert call(srv, "GET", "/api/state", token=app.token, host="evil.example:80")[0] == 403
    assert call(srv, "GET", f"/api/events?token=wrong")[0] == 403
    code, body = call(srv, "GET", "/api/state", token=app.token)
    st = json.loads(body)
    assert code == 200 and [x["slot"] for x in st["accounts"]] == [ONE, TWO]


def test_session_round_trip_over_http(gui, work):
    srv, app = gui
    code, body = call(srv, "POST", "/api/sessions", {"cwd": str(work), "model": "gemini-3.8-flash-low"}, app.token)
    sid = json.loads(body)["id"]
    assert call(srv, "POST", f"/api/sessions/{sid}/send", {"text": "hi"}, app.token)[0] == 200
    deadline = time.time() + 10
    while app.engine.sessions[sid].busy and time.time() < deadline:
        time.sleep(0.05)
    code, body = call(srv, "GET", f"/api/sessions/{sid}", token=app.token)
    assert [e["t"] for e in json.loads(body)["events"]][-1] == "turn_end"
    assert call(srv, "POST", f"/api/sessions/{sid}/send", {"text": " "}, app.token)[0] == 400
    assert call(srv, "PATCH", f"/api/sessions/{sid}", {"mode": "nope"}, app.token)[0] == 400
    assert call(srv, "GET", "/api/sessions/missing", token=app.token)[0] == 404


def test_permissions_are_agy_loops_own_and_folder_listing(gui, home):
    srv, app = gui
    code, body = call(srv, "PUT", "/api/permissions", {"allow": ["command(grep)", "command(grep)"]}, app.token)
    assert json.loads(body)["allow"] == ["command(grep)"]
    stack = json.loads((home / ".antiquota/stack/.gemini/antigravity-cli/settings.json").read_text())
    assert stack["permissions"]["allow"] == ["command(grep)"]
    assert "command(ls)" in (home / ".gemini/antigravity-cli/settings.json").read_text()  # your agy CLI is untouched
    code, body = call(srv, "GET", "/api/fs?path=/etc", token=app.token)
    assert json.loads(body)["path"] == str(home.resolve())  # outside home falls back to home


def test_add_account_from_the_gui(gui, home, monkeypatch):
    srv, app = gui
    monkeypatch.setenv("STUB_EMAIL", "three@example.com")
    code, body = call(srv, "POST", "/api/accounts/login", {}, app.token)
    lid = json.loads(body)["id"]
    assert code == 200 and json.loads(body)["url"].startswith("https://accounts.google.com/")
    assert json.loads(call(srv, "GET", f"/api/accounts/login/{lid}", token=app.token)[1])["state"] == "waiting"
    assert call(srv, "POST", f"/api/accounts/login/{lid}", {"code": "a\nb"}, app.token)[0] == 400
    code, body = call(srv, "POST", f"/api/accounts/login/{lid}", {"code": "good-code"}, app.token)
    assert json.loads(body) == {"state": "done", "error": None, "name": "three@example.com", "verify_url": None}
    slots = [x["slot"] for x in json.loads(call(srv, "GET", "/api/accounts", token=app.token)[1])]
    assert slots == [ONE, "three@example.com", TWO]
    assert call(srv, "DELETE", "/api/accounts/three@example.com", token=app.token)[0] == 200
    assert call(srv, "DELETE", "/api/accounts/..", token=app.token)[0] == 400


def test_a_new_sign_in_link_replaces_the_old_one(gui):
    srv, app = gui
    first = json.loads(call(srv, "POST", "/api/accounts/login", {}, app.token)[1])["id"]
    second = json.loads(call(srv, "POST", "/api/accounts/login", {}, app.token)[1])["id"]
    state = json.loads(call(srv, "GET", f"/api/accounts/login/{first}", token=app.token)[1])
    assert state["state"] == "failed" and "newer sign-in" in state["error"]
    assert call(srv, "POST", f"/api/accounts/login/{first}", {"code": "good-code"}, app.token)[0] == 400
    assert json.loads(call(srv, "POST", f"/api/accounts/login/{second}", {"code": "good-code"}, app.token)[1])["state"] == "done"


def test_engine_daemon_is_single_and_discoverable(home):
    import os
    import subprocess
    import sys
    from antiquota import accounts as a
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    env = {**os.environ, "PYTHONPATH": root}
    first = subprocess.Popen([sys.executable, "-m", "antiquota", "serve", "--port", "0"], env=env,
                             stdout=subprocess.PIPE, text=True)
    deadline = time.time() + 15
    while time.time() < deadline and not server.running_engine():
        time.sleep(0.1)
    info = server.running_engine()
    assert info and info["pid"] == first.pid
    with urllib.request.urlopen(f"http://127.0.0.1:{info['port']}/api/health") as r:
        assert json.loads(r.read())["app"] == "antiquota"   # no token needed to recognise it
    second = subprocess.run([sys.executable, "-m", "antiquota", "serve", "--port", "0"], env=env,
                            capture_output=True, text=True, timeout=15)
    assert second.returncode == 0 and "already runs on " + info["url"] in second.stdout
    token = open(a.state("gui-token")).read().strip()
    req = urllib.request.Request(info["url"] + "api/state", headers={"X-Antiquota-Token": token})
    with urllib.request.urlopen(req) as r:
        assert json.loads(r.read())["engine"]["busy"] == 0
    req = urllib.request.Request(info["url"] + "api/shutdown", method="POST", data=b"{}",
                                 headers={"X-Antiquota-Token": token, "Content-Type": "application/json"})
    urllib.request.urlopen(req).read()
    assert first.wait(timeout=15) == 0
    assert server.running_engine() is None and not os.path.exists(a.state("engine.json"))


def _hook(kind, payload, sid, state_dir):
    import os
    import subprocess
    import sys
    hook = os.path.join(os.path.dirname(server.__file__), "hook.py")
    r = subprocess.run([sys.executable, hook, kind], input=json.dumps(payload), capture_output=True, text=True,
                       timeout=60, env={**os.environ, "ANTIQUOTA_SESSION": sid, "ANTIQUOTA_STATE": state_dir})
    return json.loads(r.stdout)


def test_approvals_through_the_hook_and_messages_mid_turn(gui, work):
    import os
    from antiquota import accounts as a, stack
    srv, app = gui
    port = srv.server_address[1]
    a.write_atomic(a.state("engine.json"), json.dumps({"url": f"http://127.0.0.1:{port}/", "port": port}))
    state_dir = os.path.dirname(a.state("engine.json"))
    hooks = json.load(open(os.path.join(a.stack_home(), ".gemini/config/hooks.json")))
    assert hooks["antiquota"]["PreToolUse"][0]["matcher"] == "*"  # antiquota.policy sorts the tools
    sid = app.engine.create(str(work), "gemini-3.8-flash-low")["id"]
    run = lambda cmd: {"toolCall": {"name": "run_command", "args": {"CommandLine": cmd}}, "stepIdx": 3}
    # antiquota's allow rules answer at once (the stack allows `ls`); reading needs nobody's yes
    assert _hook("pre-tool", run("ls -la"), sid, state_dir) == {"decision": "allow"}
    assert _hook("pre-tool", {"toolCall": {"name": "view_file", "args": {}}}, sid, state_dir) == {"decision": "allow"}
    assert _hook("pre-tool", run("ls"), "", state_dir) == {"decision": "ask"}  # not an antiquota turn
    # anything else waits for the user in the GUI
    out = {}
    t = threading.Thread(target=lambda: out.update(_hook("pre-tool", run("npm test -- --watch=false"), sid, state_dir)))
    t.start()
    deadline = time.time() + 10
    while time.time() < deadline and not app.engine.approvals:
        time.sleep(0.05)
    (ap,) = app.engine.approvals.values()
    assert ap["rule"] == "command(npm test)" and app.engine.sessions[sid].meta["status"] == "approval"
    call(srv, "POST", f"/api/approvals/{ap['id']}/answer", {"decision": "allow", "always": True}, token=app.token)
    t.join(10)
    assert out == {"decision": "allow"}
    assert "command(npm test)" in stack.permissions()[0]  # "always" became an allow rule
    assert _hook("pre-tool", run("npm test"), sid, state_dir) == {"decision": "allow"}
    # declined, and edits in plan mode
    t = threading.Thread(target=lambda: out.update(_hook("pre-tool", run("rm -rf build"), sid, state_dir)))
    t.start()
    while len(app.engine.approvals) < 2:
        time.sleep(0.05)
    pending = [x for x in app.engine.approvals.values() if x["state"] == "pending"][0]
    call(srv, "POST", f"/api/approvals/{pending['id']}/answer", {"decision": "deny"}, token=app.token)
    t.join(10)
    assert out["decision"] == "deny" and "declined" in out["reason"]
    app.engine.update(sid, {"mode": "plan"})
    edit = {"toolCall": {"name": "write_to_file", "args": {"TargetFile": str(work / "a.py")}}}
    assert _hook("pre-tool", edit, sid, state_dir)["decision"] == "deny"
    app.engine.update(sid, {"mode": "accept-edits"})
    assert _hook("pre-tool", edit, sid, state_dir) == {"decision": "allow"}
    # a message sent while agy works reaches it at its next model call
    app.engine.sessions[sid].meta["queue"] = ["also update the README", {"text": "with file", "attachments": []}]
    assert _hook("pre-invocation", {}, sid, state_dir) == {"injectSteps": [{"userMessage": "also update the README"}]}
    assert app.engine.sessions[sid].meta["queue"] == [{"text": "with file", "attachments": []}]
    assert _hook("pre-invocation", {}, sid, state_dir) == {}
    assert [e.get("steered") for e in app.engine.events(sid) if e["t"] == "user"] == [True]
    # nobody to ask (engine gone): the hook decides with the rules and declines the rest
    a.write_atomic(a.state("engine.json"), json.dumps({"url": "http://127.0.0.1:9/", "port": 9}))
    assert _hook("pre-tool", run("ls"), sid, state_dir) == {"decision": "allow"}
    gone = _hook("pre-tool", run("curl example.com"), sid, state_dir)
    assert gone["decision"] == "deny" and "not running" in gone["reason"]


def test_transcribe_route_needs_audio(gui):
    srv, app = gui
    code, body = call(srv, "POST", "/api/transcribe", token=app.token)
    assert code == 400 and b"record" in body
