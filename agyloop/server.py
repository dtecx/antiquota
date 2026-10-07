"""Local web GUI: stdlib HTTP + Server-Sent Events over the engine.

Bound to 127.0.0.1 only. Every API call needs the per-launch token (header; the
query string only for EventSource) and a localhost Host header, so other web
pages and DNS-rebinding tricks cannot drive agy through it.
"""
import json
import os
import queue
import secrets
import subprocess
import threading
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

from . import accounts as a
from . import stack
from .engine import MODES, Engine

UI = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ui")
STATIC = {"/": "index.html", "/app.js": "app.js", "/app.css": "app.css"}
TYPES = {".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8",
         ".css": "text/css; charset=utf-8"}
PROBE_EVERY = 600


def settings_path():
    """agy-loop's own permission rules; your agy CLI keeps its own."""
    a.seed_stack()
    return os.path.join(a.stack_home(), ".gemini/antigravity-cli/settings.json")


def gui_token():
    """Per-install token, kept across restarts so an open tab survives a server restart."""
    p = a.state("gui-token")
    try:
        with open(p) as f:
            tok = f.read().strip()
        if len(tok) >= 32:
            return tok
    except OSError:
        pass
    tok = secrets.token_urlsafe(24)
    fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(tok)
    return tok


class App:
    def __init__(self, port):
        self.port = port
        self.token = gui_token()
        self.engine = Engine()
        self.models = []
        self.stack = None
        self.logins = {}
        threading.Thread(target=self._background, daemon=True).start()

    def _models(self):
        """Model list as an agy-loop account sees it (your own agy CLI is never asked)."""
        if self.models or not a.slots():
            return
        try:
            cmd, env = a.agy_cmd(a.slot_home(a.slots()[0]), ["models"])
            out = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=60,
                                 stdin=subprocess.DEVNULL).stdout
            self.models = [dict(zip(("id", "label"), line.split("\t", 1))) for line in out.splitlines() if "\t" in line]
        except (OSError, subprocess.TimeoutExpired, SystemExit):
            pass

    def _background(self):
        while True:  # keep walls honest while the GUI runs; cron covers the rest of the time
            self.probe()
            self._models()
            threading.Event().wait(PROBE_EVERY)

    def accounts(self):
        out, today = [], a.usage_today()
        for s in a.slots():
            try:
                with open(a.state("quota", s + ".json")) as f:
                    q = json.load(f)
            except (OSError, ValueError):
                q = {}
            fd = a.try_lease(s)
            if fd is not None:
                os.close(fd)
            out.append({"slot": s, "busy": fd is None, "probed": q.get("at"), "quota": q.get("quota", {}),
                        "walls": {b: a.walled_until(s, b) for b in a.BUCKETS}, "verify": a.needs_verify(s),
                        "today": today.get(s, {})})
        return out

    def check(self, name):
        """Probe one account now (e.g. right after verifying it with Google)."""
        if name not in a.slots():
            raise KeyError(name)
        fd = a.try_lease(name)
        if fd is None:
            raise ValueError(f"{name} is running a call; it is checked when that ends")
        try:
            a.sync(a.slot_home(name))
            a.probe_slot(name)
        finally:
            os.close(fd)
        accts = self.accounts()
        self.engine.publish(None, {"t": "accounts", "accounts": accts})
        return accts

    def probe(self):
        for s in a.slots():
            fd = a.try_lease(s)
            if fd is None:
                continue  # busy: its walls come from the running call
            try:
                a.sync(a.slot_home(s))
                a.probe_slot(s)
            finally:
                os.close(fd)
        self._models()
        self.engine.publish(None, {"t": "accounts", "accounts": self.accounts()})

    def start_login(self):
        for k, lg in list(self.logins.items()):  # one sign-in at a time: a new link replaces the old
            if not lg.ended.is_set():
                lg.cancel("replaced by a newer sign-in link (only the newest one works)")
            del self.logins[k]
        lg = a.Login()
        url = lg.wait_url(20)
        if not url:
            lg.cancel()
            raise ValueError(lg.error or "agy printed no sign-in URL")
        lid = secrets.token_hex(8)
        self.logins[lid] = lg

        def announce():
            lg.ended.wait()
            if lg.state in ("done", "verify"):
                self.probe()
        threading.Thread(target=announce, daemon=True).start()
        return {"id": lid, "url": url, "seconds": 60}

    def stack_info(self, refresh=False, reseed=False):
        if reseed:
            a.seed_stack(force=True)
        if self.stack is None or refresh or reseed:
            a.seed_stack()
            try:
                with open(os.path.join(a.stack_home(), ".gemini/config/mcp_config.json")) as f:
                    mcp = sorted(json.load(f).get("mcpServers", {}))
            except (OSError, ValueError):
                mcp = []
            self.stack = {**a.loaded(), "mcp": mcp, "path": os.path.join(a.stack_home(), ".gemini")}
        return self.stack

    def permissions(self):
        try:
            with open(settings_path()) as f:
                return json.load(f).get("permissions", {}).get("allow", [])
        except (OSError, ValueError):
            return []

    def set_permissions(self, allow=None, deny=None):
        try:
            with open(settings_path()) as f:
                d = json.load(f)
        except (OSError, ValueError):
            d = {}
        perms = d.setdefault("permissions", {})
        for key, rules in (("allow", allow), ("deny", deny)):
            if rules is None:
                continue
            if not isinstance(rules, list) or not all(isinstance(r, str) and 0 < len(r) < 300 for r in rules):
                raise ValueError(f"{key} must be a list of rule strings")
            perms[key] = list(dict.fromkeys(rules))
        with open(settings_path() + ".tmp", "w") as f:
            json.dump(d, f, indent=2)
        os.replace(settings_path() + ".tmp", settings_path())
        return perms

    def listdir(self, path):
        home = os.path.realpath(a.home())
        p = os.path.realpath(os.path.expanduser(path or home))
        if not (p == home or p.startswith(home + os.sep)) or not os.path.isdir(p):
            p = home
        dirs = sorted((n for n in os.listdir(p) if not n.startswith(".") and os.path.isdir(os.path.join(p, n))),
                      key=str.lower)
        return {"path": p, "parent": os.path.dirname(p) if p != home else None, "dirs": dirs}

    def state(self):
        e = self.engine
        return {"sessions": sorted((s.meta for s in e.sessions.values()), key=lambda m: -m["updated"]),
                "accounts": self.accounts(), "models": self.models, "modes": list(MODES),
                "defaults": {**stack.defaults(), "cwd": a.home()}}


class Handler(BaseHTTPRequestHandler):
    app = None
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def do_GET(self):
        self._route("GET")

    def do_POST(self):
        self._route("POST")

    def do_PATCH(self):
        self._route("PATCH")

    def do_PUT(self):
        self._route("PUT")

    def do_DELETE(self):
        self._route("DELETE")

    def _send(self, code, body, ctype):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(body)

    def _json(self, code, obj):
        self._send(code, json.dumps(obj).encode(), "application/json")

    def _body(self):
        n = int(self.headers.get("Content-Length") or 0)
        if n > 5_000_000:
            raise ValueError("body too large")
        return json.loads(self.rfile.read(n) or b"{}") if n else {}

    def _route(self, method):
        app, u = self.app, urlparse(self.path)
        qs = parse_qs(u.query)
        if self.headers.get("Host") not in (f"127.0.0.1:{app.port}", f"localhost:{app.port}"):
            return self._json(403, {"error": "bad host"})
        if method == "GET" and u.path in STATIC:
            name = STATIC[u.path]
            with open(os.path.join(UI, name), "rb") as f:
                body = f.read()
            if name == "index.html":
                body = body.replace(b"__AGYLOOP_TOKEN__", app.token.encode())
            return self._send(200, body, TYPES[os.path.splitext(name)[1]])
        token = self.headers.get("X-Agyloop-Token", "")
        if u.path == "/api/events":
            token = (qs.get("token") or [""])[0]
        if not u.path.startswith("/api/") or not secrets.compare_digest(token, app.token):
            return self._json(403, {"error": "forbidden"})
        try:
            if u.path == "/api/events":
                return self._events()
            body = self._body() if method in ("POST", "PATCH", "PUT") else {}
            return self._json(200, self._api(method, u.path.split("/")[2:], body, qs))
        except KeyError as e:
            return self._json(404, {"error": f"not found: {e}"})
        except ValueError as e:
            return self._json(400, {"error": str(e)})

    def _api(self, method, parts, b, qs):
        app, e = self.app, self.app.engine
        match (method, parts):
            case ("GET", ["state"]):
                return app.state()
            case ("POST", ["sessions"]):
                d = stack.defaults()
                return e.create(b.get("cwd") or a.home(), b.get("model") or d["model"], "",
                                b.get("mode") or d["mode"], b.get("title", ""))
            case ("GET", ["sessions", sid]):
                return {"meta": e.sessions[sid].meta, "events": e.events(sid)}
            case ("PATCH", ["sessions", sid]):
                return e.update(sid, b)
            case ("DELETE", ["sessions", sid]):
                e.delete(sid)
                return {"ok": True}
            case ("POST", ["sessions", sid, "send"]):
                if not str(b.get("text", "")).strip():
                    raise ValueError("empty message")
                return {"result": e.send(sid, b["text"])}
            case ("POST", ["sessions", sid, "stop"]):
                e.stop(sid)
                return {"ok": True}
            case ("POST", ["sessions", sid, "goal"]):
                if not str(b.get("objective", "")).strip():
                    raise ValueError("empty objective")
                return e.set_goal(sid, b["objective"], b.get("check", ""), b.get("verifier", True),
                                  b.get("verifier_model", ""), b.get("max_rounds", 20))
            case ("POST", ["sessions", sid, "goal", "resume"]):
                e.resume_goal(sid)
                return {"ok": True}
            case ("DELETE", ["sessions", sid, "goal"]):
                e.clear_goal(sid)
                return {"ok": True}
            case ("GET", ["accounts"]):
                return app.accounts()
            case ("POST", ["accounts", "probe"]):
                threading.Thread(target=app.probe, daemon=True).start()
                return {"ok": True}
            case ("POST", ["accounts", "login"]):
                return app.start_login()
            case ("GET", ["accounts", "login", lid]):
                if lid not in app.logins:
                    return {"state": "failed", "error": "replaced by a newer sign-in link (only the newest one works)"}
                lg = app.logins[lid]
                return {"state": lg.state, "error": lg.error, "name": lg.name, "verify_url": lg.verify_url}
            case ("POST", ["accounts", "login", lid]):
                if lid not in app.logins:
                    raise ValueError("this sign-in was replaced by a newer link; use the newest one")
                lg = app.logins[lid]
                lg.submit(str(b.get("code", "")))
                return {"state": lg.state, "error": lg.error, "name": lg.name, "verify_url": lg.verify_url}
            case ("DELETE", ["accounts", "login", lid]):
                if lid in app.logins:
                    app.logins.pop(lid).cancel()
                return {"ok": True}
            case ("POST", ["accounts", name, "check"]):
                return app.check(name)
            case ("DELETE", ["accounts", name]):
                a.remove(name)
                threading.Thread(target=app.probe, daemon=True).start()
                return {"ok": True}
            case ("GET", ["settings"]):
                return {**stack.overview(), "defaults": stack.defaults()}
            case ("PUT", ["settings", "defaults"]):
                return stack.set_defaults(b)
            case ("GET", ["settings", "file"]):
                return stack.read_file((qs.get("path") or [""])[0])
            case ("PUT", ["settings", "file"]):
                stack.write_file(str(b.get("path", "")), b.get("text"))
                app.stack = None
                return {"ok": True}
            case ("GET", ["settings", kind, name]):
                return stack.get_item(kind, name)
            case ("PUT", ["settings", kind, name]):
                stack.put_item(kind, name, b.get("value"))
                app.stack = None
                return {"ok": True}
            case ("DELETE", ["settings", kind, name]):
                stack.delete(kind, name)
                app.stack = None
                return {"ok": True}
            case ("POST", ["settings", kind, name, "enabled"]):
                stack.set_enabled(kind, name, bool(b.get("on")))
                app.stack = None
                return {"ok": True}
            case ("POST", ["settings", "copy"]):
                stack.copy(str(b.get("source", "")), str(b.get("kind", "")), b.get("name") or None)
                app.stack = None
                return {"ok": True}
            case ("GET", ["stack"]):
                return app.stack_info(refresh="refresh" in qs)
            case ("POST", ["stack", "reseed"]):
                return app.stack_info(reseed=True)
            case ("GET", ["permissions"]):
                return {"allow": app.permissions()}
            case ("PUT", ["permissions"]):
                return app.set_permissions(b.get("allow"), b.get("deny"))
            case ("GET", ["fs"]):
                return app.listdir((qs.get("path") or [""])[0])
        raise KeyError("/".join(parts))

    def _events(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.close_connection = True
        q = self.app.engine.subscribe()
        try:
            while True:
                try:
                    data = f"data: {json.dumps(q.get(timeout=15))}\n\n"
                except queue.Empty:
                    data = ": ping\n\n"
                self.wfile.write(data.encode())
                self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass
        finally:
            self.app.engine.unsubscribe(q)


def make_server(port):
    srv = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    srv.daemon_threads = True
    Handler.app = App(srv.server_address[1])  # port 0 picks a free one
    return srv


def serve(port=8765, open_browser=True):
    srv = make_server(port)
    url = f"http://127.0.0.1:{port}/"
    print(f"agyloop GUI on {url}  (Ctrl-C to stop)")
    if open_browser:
        webbrowser.open(url)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
