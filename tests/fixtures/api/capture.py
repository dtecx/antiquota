#!/usr/bin/env python3
"""Capture the running legacy server using isolated fake accounts and real HTTP."""
import ast
import base64
import datetime
import hashlib
import http.client
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import threading
import time
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[3]
CASE_ROOT = ROOT / "tests/fixtures/api"
DATA_ROOT = ROOT / "tests/fixtures/data"
RAW_ROOT = ROOT / ".handoff/evidence/api-raw"
TEST_ROOT = Path.home() / "Desktop/test/antiquota-contract"
HOME = TEST_ROOT / "home"
BASE = TEST_ROOT / "data"
PROJECT = HOME / "project"
sys.path.insert(0, str(ROOT))
os.environ.update(ANTIQUOTA_HOME=str(HOME), ANTIQUOTA_DIR=str(BASE), ANTIQUOTA_NO_USAGE_LINK="1")
for key in list(os.environ):
    if key.startswith("STUB_"):
        del os.environ[key]

def setup():
    if TEST_ROOT.exists():
        shutil.rmtree(TEST_ROOT)
    HOME.mkdir(parents=True)
    BASE.mkdir()
    PROJECT.mkdir()
    entries = {
        ".gemini/GEMINI.md": "Use the isolated project.\n",
        ".gemini/config/hooks.json": "{}",
        ".gemini/config/mcp_config.json": '{"mcpServers": {}}',
        ".gemini/config/skills/example/SKILL.md": "---\ndescription: Example skill\n---\nFollow this example.\n",
        ".gemini/config/rules/example.md": "Keep changes small.\n",
        ".gemini/config/plugins/example/README.md": "Fixture plugin resources.\n",
        ".gemini/config/config.json": '{"plugins":{"example":{"enabled":true}}}',
        ".gemini/antigravity-cli/settings.json": '{"permissions": {"allow": ["command(ls)"], "deny": []}}',
        ".gemini/antigravity-cli/installation_id": "fixture-installation",
    }
    for rel, text in entries.items():
        path = HOME / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    wrapper = TEST_ROOT / "fake-agy"
    wrapper.write_text("#!/usr/bin/env python3\nimport json,os,sys\na=sys.argv[1:]\n"
        "if len(a)>1 and a[0]=='-p' and a[1].startswith('Listen to the audio file '):\n"
        " print(json.dumps({'status':'SUCCESS','response':os.environ.get('STUB_DICTATION','Captured speech.')}));sys.exit(0)\n"
        "os.execv(sys.executable,[sys.executable," + repr(str(ROOT / "tests/stub_agy.py")) + ",*a])\n")
    wrapper.chmod(0o755)
    os.environ["ANTIQUOTA_AGY"] = str(wrapper)
    (BASE / "settings.json").write_text('{"keep_awake":"off"}')
    (PROJECT / "readme.txt").write_text("Contract fixture project.\n")
    for args in (["init", "-q", "-b", "main"], ["config", "user.name", "Fixture Author"],
                 ["config", "user.email", "fixture@example.com"], ["add", "."], ["commit", "-qm", "Initial fixture"]):
        subprocess.run(["git", *args], cwd=PROJECT, check=True, capture_output=True)
    for directory in (CASE_ROOT / "cases", RAW_ROOT):
        directory.mkdir(parents=True, exist_ok=True)
    for path in (CASE_ROOT / "cases").glob("*.json"):
        path.unlink()

setup()
from antiquota import accounts as a, server

for email in ("one@example.com", "two@example.com"):
    os.environ["STUB_EMAIL"] = email
    os.environ["STUB_ACCOUNT"] = email + "-fixture-token"
    login = a.Login()
    assert login.wait_url(5), "fake sign-in URL missing"
    login.submit("good-code", timeout=5)
    assert login.state == "done", login.error

srv = server.make_server(0)
server.Handler.server_ref = srv
app = server.Handler.app
PORT = srv.server_address[1]
URL = f"http://127.0.0.1:{PORT}"
a.write_atomic(a.state("engine.json"), json.dumps({"pid": os.getpid(), "port": PORT, "url": URL + "/",
    "version": "0.5.0", "started": time.time(), "by": "contract-capture"}))
threading.Thread(target=srv.serve_forever, daemon=True).start()

aliases = {app.token: "{{TOKEN}}", str(TEST_ROOT): "{{TEST_ROOT}}", str(ROOT): "{{REPO_ROOT}}",
           str(PORT): "{{PORT}}"}
counters = {}
records = []
captured_disk = {}
disk_provenance = {}

def alias(value, kind):
    if value not in aliases:
        counters[kind] = counters.get(kind, 0) + 1
        aliases[value] = "{{" + kind + "_" + str(counters[kind]) + "}}"
    return value

def discover(value):
    if isinstance(value, dict):
        for key, val in value.items():
            if isinstance(val, str):
                if key in ("id", "pending") and len(val) in (8, 10, 12, 16):
                    alias(val, {8: "JOB", 10: "APPROVAL", 12: "SESSION", 16: "LOGIN"}[len(val)])
                if key == "conversation_id" and val:
                    alias(val, "CONVERSATION")
                if key in ("sha", "base_sha") and val:
                    alias(val, "COMMIT")
                if key == "branch" and val.startswith("antiquota/"):
                    alias(val, "BRANCH")
            discover(val)
    elif isinstance(value, list):
        for item in value:
            discover(item)

def normalize(value):
    if isinstance(value, str):
        for src, dst in sorted(aliases.items(), key=lambda item: -len(item[0])):
            if src == str(PORT):
                value = value.replace(":" + src, ":" + dst)
            else:
                value = value.replace(src, dst)
        return value
    if isinstance(value, list):
        return [normalize(v) for v in value]
    if isinstance(value, dict):
        return {normalize(k): normalize(v) for k, v in value.items()}
    return value

def capture_stack(name, relative_path, phase):
    """Retain actual owned stack bytes at a named point in the HTTP scenario."""
    path = BASE / "stack/.gemini" / relative_path
    data = path.read_bytes()
    captured_disk[name] = data.decode("utf-8")
    disk_provenance[name] = {"source": "stack/.gemini/" + relative_path, "phase": phase,
        "encoding": "UTF-8", "rawSha256": hashlib.sha256(data).hexdigest(), "normalization": "String path/identity placeholders only; preserve source whitespace and bytes otherwise."}

class TrackedConnection(http.client.HTTPConnection):
    """Record the headers actually emitted by the HTTP library, including defaults."""
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.sent_headers = {}

    def putheader(self, header, *values):
        self.sent_headers[header] = " ".join(map(str, values))
        super().putheader(header, *values)

def request(name, method, path, body=None, raw=None, headers=None, auth=True, declared_size=None):
    data = raw if raw is not None else json.dumps(body).encode() if body is not None else None
    hdr = {"X-Antiquota-Token": app.token} if auth else {}
    if body is not None:
        hdr["Content-Type"] = "application/json"
    if headers:
        hdr.update(headers)
    if declared_size is not None:
        hdr["Content-Length"] = str(declared_size)
    conn = TrackedConnection("127.0.0.1", PORT, timeout=12)
    try:
        conn.request(method, path, body=data, headers=hdr)
        res = conn.getresponse()
        payload = res.read()
        response = {"status": res.status, "headers": dict(res.getheaders())}
        ctype = res.getheader("Content-Type", "")
        if "application/json" in ctype:
            response["json"] = json.loads(payload)
            discover(response["json"])
        else:
            try:
                response["text"] = payload.decode("utf-8")
            except UnicodeDecodeError:
                response["base64"] = base64.b64encode(payload).decode()
        rec = {"name": name, "request": {"method": method, "path": path, "headers": conn.sent_headers}, "response": response}
        if body is not None:
            rec["request"]["json"] = body
        elif raw is not None:
            rec["request"]["base64"] = base64.b64encode(raw).decode()
        if declared_size is not None:
            rec["request"]["declaredContentLength"] = declared_size
        records.append(rec)
        (RAW_ROOT / (name + ".json")).write_text(json.dumps(rec, indent=2) + "\n")
        print(f"{name}: {res.status}", flush=True)
        return response.get("json", payload)
    except Exception as exc:
        rec = {"name": name, "request": {"method": method, "path": path, "headers": conn.sent_headers},
               "transportError": type(exc).__name__ + ": " + str(exc)}
        if body is not None:
            rec["request"]["json"] = body
        records.append(rec)
        print(f"{name}: transport error {type(exc).__name__}", flush=True)
        return None
    finally:
        conn.close()

def call(name, method, path, body=None, **kwargs):
    return request(name, method, "/api/" + path, body, **kwargs)

def wait_idle(sid, seconds=15):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if not app.engine.sessions[sid].busy:
            return
        time.sleep(0.025)
    raise RuntimeError("fake turn did not become idle")

def stream(name, path):
    conn = TrackedConnection("127.0.0.1", PORT, timeout=18)
    conn.request("GET", path)
    response = conn.getresponse()
    record = {"name": name, "request": {"method": "GET", "path": path, "headers": conn.sent_headers},
              "response": {"status": response.status, "headers": dict(response.getheaders()), "events": []}}
    stop = threading.Event()
    def collect():
        current = []
        try:
            while not stop.is_set():
                line = response.fp.readline().decode("utf-8")
                if not line:
                    break
                if line.strip():
                    current.append(line.rstrip("\n"))
                elif current:
                    entry = "\n".join(current)
                    record["response"]["events"].append(entry)
                    if entry.startswith("data: "):
                        discover(json.loads(entry[6:]))
                    current = []
        except (OSError, AttributeError):
            pass
    thread = threading.Thread(target=collect, daemon=True)
    thread.start()
    records.append(record)
    return conn, stop, thread, record

events_conn, events_stop, events_thread, events_record = stream("events-sse", "/api/events?token=" + app.token)

from scenario import run

try:
    run(sys.modules[__name__])
finally:
    events_stop.set()
    events_conn.close()
    app.engine.shutdown()
    srv.shutdown()
    srv.server_close()

DATA_ROOT.mkdir(parents=True, exist_ok=True)
disk_examples = {}
disk_examples.update({name: normalize(value) for name, value in captured_disk.items()})
for rel in ("settings.json", "state/engine.json", "state/routing.jsonl", "state/usage.jsonl", "state/quota/one@example.com.json"):
    path = BASE / rel
    if path.is_file():
        text = path.read_text()
        value = [json.loads(line) for line in text.splitlines() if line.strip()] if path.suffix == ".jsonl" else json.loads(text)
        disk_examples[rel.replace("/", "-").replace(".jsonl", ".json")] = normalize(value)
for name, field, output in (("session-get-empty", "meta", "session-empty.json"),
        ("session-get-turn", "events", "session-events.json"), ("goal-get-met", "meta", "session-goal-met.json"),
        ("state-final", "sessions", "sessions-final.json")):
    record = next(r for r in records if r["name"] == name)
    disk_examples[output] = normalize(record["response"]["json"][field])
a.set_wall("one@example.com", "gemini", 4070908800, "fixture wall")
a.mark_verify("one@example.com", "https://accounts.google.com/signin/continue?fixture=1")
a.set_paused("one@example.com", True)
disk_examples["state-wall.txt"] = normalize(Path(a.state("walls", "one@example.com.gemini")).read_text())
disk_examples["state-verification.json"] = normalize(json.loads(Path(a.state("verify", "one@example.com.json")).read_text()))
for name, value in disk_examples.items():
    text = value if isinstance(value, str) else json.dumps(value, indent=2) + "\n"
    (DATA_ROOT / name).write_text(text)
links = []
for path in sorted((BASE / "slots/one@example.com").rglob("*")):
    if path.is_symlink():
        links.append({"path": str(path.relative_to(BASE)), "target": normalize(os.readlink(path)), "exists": path.exists()})
(DATA_ROOT / "slot-links.json").write_text(json.dumps(links, indent=2) + "\n")
(DATA_ROOT / "manifest.json").write_text(json.dumps({"format": 1, "stackSnapshots": disk_provenance,
    "checkpointSource": "sessions/<session-create id>/checkpoints/1.json immediately after send completes",
    "sessionExamples": "Exact JSON fields captured from legacy HTTP responses, identical to the meta/event disk writers",
    "stateExamples": "Read from isolated data files after HTTP shutdown; verification/wall markers written by the legacy account helpers",
    "credentials": "Only the read-only structural schema is committed; no token values."}, indent=2) + "\n")

for record in records:
    if "events" in record.get("response", {}):
        record["response"]["eventSamples"] = [json.loads(line[6:]) for line in record["response"]["events"] if line.startswith("data: ")]
        (RAW_ROOT / (record["name"] + ".json")).write_text(json.dumps(record, indent=2) + "\n")
    normalized = normalize(record)
    def pointer_escape(text):
        return str(text).replace("~", "~0").replace("/", "~1")
    dynamic = {}
    def dynamic_paths(value, path="/response", key=""):
        if isinstance(value, dict):
            for k, v in value.items():
                dynamic_paths(v, path + "/" + pointer_escape(k), k)
        elif isinstance(value, list):
            for i, v in enumerate(value):
                dynamic_paths(v, path + "/" + str(i), "")
        elif isinstance(value, (int, float)) and not isinstance(value, bool) and key in {
                "ts", "created", "updated", "started", "ended", "at", "probed", "deadline", "answered", "secs", "next", "status_until", "pid"}:
            dynamic[path] = {"type": "positive-integer" if key == "pid" else "nonnegative-number", "meaning": "process identifier" if key == "pid" else "duration seconds" if key == "secs" else "timestamp"}
    dynamic_paths(normalized.get("response", {}))
    normalized["comparison"] = {"dynamicValues": dynamic,
        "headers": {"Date": "valid HTTP-date", "Server": "implementation-specific server identity", "Content-Length": "must equal actual encoded response body byte length"},
        "sse": "Compare parsed eventSamples; raw events retain captured framing evidence." if "events" in normalized.get("response", {}) else None}
    if record["name"] == "terminal-sse":
        normalized["comparison"]["terminal"] = {"chunks": "Decode and concatenate all d fields in order; chunk boundaries are unconstrained.",
            "metadata": "First frame shell/cwd/alive fields are required.", "completion": "An exit=true event terminates the stream.",
            "output": "Assert the typed command and contract-terminal result are present; shell prompt bytes are platform-specific."}
    if "usage" in record["name"]:
        normalized["comparison"]["calendar"] = "days[*].date is the replay local date minus the requested number of days plus row index; calendar dates shift with the replay clock."
    (CASE_ROOT / "cases" / (record["name"] + ".json")).write_text(json.dumps(normalized, indent=2) + "\n")

source = ast.parse((ROOT / "antiquota/server.py").read_text())
routes = []
for node in ast.walk(source):
    if isinstance(node, ast.Match):
        for case in node.cases:
            method_node, parts_node = case.pattern.patterns
            method = method_node.value.value
            parts = []
            for part in parts_node.patterns:
                parts.append(part.value.value if isinstance(part, ast.MatchValue) else "{" + part.name + "}")
            routes.append({"method": method, "path": "/api/" + "/".join(parts)})
for index, route in enumerate(routes):
    route["featureId"] = f"F-{329 + index:03d}"
routes += [{"method": "GET", "path": "/api/health"}, {"method": "GET", "path": "/api/events"},
    {"method": "GET", "path": "/api/sessions/{sid}/term/stream"}, {"method": "POST", "path": "/api/transcribe"},
    {"method": "POST", "path": "/api/sessions/{sid}/attach"},
    {"method": "GET", "path": "/api/sessions/{sid}/attachments/{name}"}]
for index, route in enumerate(routes[-6:]):
    route["featureId"] = f"F-{323 + index:03d}"
def matches(route, record):
    request = record["request"]
    if request["method"] != route["method"]:
        return False
    actual = request["path"].split("?", 1)[0].split("/")
    target = route["path"].split("/")
    return len(actual) == len(target) and all(t.startswith("{") or t == v for t, v in zip(target, actual))
for route in routes:
    route["fixtures"] = [r["name"] for r in records if matches(route, r)]
    route["successFixtures"] = [r["name"] for r in records if matches(route, r) and r.get("response", {}).get("status") == 200]
manifest = {"format": 1, "capturedAt": datetime.datetime.now().isoformat(), "sourceCommit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
    "sourceSha256": hashlib.sha256((ROOT / "antiquota/server.py").read_bytes()).hexdigest(),
    "environment": "Real Python ThreadingHTTPServer, isolated HOME/data, existing fake agy plus dictation JSON scenario", "paidCalls": 0,
    "routes": routes, "caseCount": len(records), "replaySequence": [r["name"] for r in records], "normalization": {"stringPlaceholders": sorted(set(aliases.values())),
        "dynamicValues": "Each case lists exact JSON-pointer paths and type constraints; no global key skipping.",
        "responseHeaderValuePatterns": {"Date": "HTTP-date", "Server": "BaseHTTP/version Python/version"}},
    "transportErrors": [r["name"] for r in records if "transportError" in r]}
binding_sources = []
for name, pointer in (("session-create", "/response/json/id"), ("session-fork", "/response/json/id"),
        ("worktree-create", "/response/json/id"), ("login-start", "/response/json/id"),
        ("approval-request", "/response/json/pending"), ("job-create", "/response/json/id"),
        ("approval-request-allow", "/response/json/pending"),
        ("session-get-turn", "/response/json/meta/conversation_id"), ("session-git-commit", "/response/json/sha"),
        ("session-patch-default", "/response/json/conversation_id"),
        ("worktree-create", "/response/json/worktree/base_sha"), ("worktree-commit", "/response/json/sha"),
        ("worktree-create", "/response/json/worktree/branch"),
        ("worktree-create", "/response/json/cwd"),
        ("worktree-merge", "/response/json/sha")):
    record = next(r for r in records if r["name"] == name)
    value = record
    for key in pointer.strip("/").split("/"):
        value = value[key]
    binding_sources.append({"placeholder": "{{WORKTREE_PATH}}" if pointer == "/response/json/cwd" else normalize(value), "fixture": name, "pointer": pointer,
        "bindOnly": pointer == "/response/json/cwd"})
manifest["bindings"] = binding_sources
manifest["calendar"] = {"capturedLocalDate": datetime.datetime.now().date().isoformat(),
    "zone": os.path.realpath("/etc/localtime").split("/zoneinfo/")[-1], "observedOffsetSeconds": int(datetime.datetime.now().astimezone().utcoffset().total_seconds()),
    "replay": "Use a fake clock at captured local date with this timezone, or remap usage day strings relative to replay local date while keeping row order/totals exact."}
manifest["setup"] = {"script": "capture.py", "scenario": "scenario.py", "directory": "{{TEST_ROOT}}",
    "home": "{{TEST_ROOT}}/home", "data": "{{TEST_ROOT}}/data", "project": "{{TEST_ROOT}}/home/project",
    "initialAccounts": ["one@example.com", "two@example.com"], "quotaScenario": "tests/stub_agy.py: 90% both pools and windows, fixed 2099 resets",
    "externalProcesses": "Only the fake agy, local git, and a local PTY shell; no network accounts or VPS calls.",
    "nonHttpSteps": [{"before": "worktree-git", "action": "write {{WORKTREE_PATH}}/worktree.txt", "text": "Worktree change.\n"},
        {"before": "thinking-send", "action": "Create fake agy conversation SQLite steps row idx4/type15 with field20.3 thinking; exact writer is in scenario.py."}],
    "environmentTransitions": [{"before": "send-long", "set": {"STUB_SLEEP": "1.5"}}, {"after": "session-steer", "wait": "session idle", "unset": ["STUB_SLEEP"]},
        {"before": "transcribe-no-speech", "set": {"STUB_DICTATION": "NO_SPEECH"}}, {"before": "login-start", "set": {"STUB_EMAIL": "three@example.com"}},
        {"before": "quota-handoff-send", "set": {"STUB_429": "current session last_slot"}}, {"after": "quota-handoff-send", "wait": "session idle", "unset": ["STUB_429"]}],
    "waits": "Wait for session idle after send, session-steer, goal-create and goal-resume; wait for quota probe lease release before account-check-idle."}
(CASE_ROOT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps({"caseCount": len(records), "routes": len(routes), "missing": [r for r in routes if not r["successFixtures"]], "transportErrors": manifest["transportErrors"]}), flush=True)
