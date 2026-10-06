import json
import threading
import time
import urllib.error
import urllib.request

import pytest

from agyloop import server

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
        req.add_header("X-Agyloop-Token", token)
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
    stack = json.loads((home / ".agyloop/stack/.gemini/antigravity-cli/settings.json").read_text())
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
    assert json.loads(body) == {"state": "done", "error": None, "name": "three@example.com"}
    slots = [x["slot"] for x in json.loads(call(srv, "GET", "/api/accounts", token=app.token)[1])]
    assert slots == [ONE, "three@example.com", TWO]
    assert call(srv, "DELETE", "/api/accounts/three@example.com", token=app.token)[0] == 200
    assert call(srv, "DELETE", "/api/accounts/..", token=app.token)[0] == 400
