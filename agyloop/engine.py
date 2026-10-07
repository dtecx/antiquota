"""Sessions and turns: agy conversations driven headless across accounts.

Every turn is one `agy -p ... --output-format stream-json` process on the first
account with quota left. A quota error walls that account and the same turn
continues on the next one, in the same conversation (agy's conversation store
is shared by every account). Goal mode keeps pushing until the agent claims the
objective is done AND an optional check command and verifier agree.

Events go to subscribers (the GUI, the CLI) and, when final, to
<base>/sessions/<id>/events.jsonl, so a session survives a restart.
"""
import difflib
import json
import os
import queue
import re
import shutil
import signal
import subprocess
import threading
import time
import uuid
from datetime import datetime

from . import accounts as a
from . import convdb

EDIT_TOOLS = {"write_to_file", "replace_file_content", "multi_replace_file_content", "sed_file"}
MODES = {"default": [], "accept-edits": ["--mode", "accept-edits"], "plan": ["--mode", "plan"],
         "bypass": ["--dangerously-skip-permissions"]}
DONE_MARK = "GOAL_COMPLETE"
OUT_CAP = 20000
# agy ignores --json-schema for these calls, so the verifier ends with a plain verdict line.
VERDICT_RE = re.compile(r"^\W*VERDICT:\s*(DONE|NOT[_ ]DONE)\W*$", re.I | re.M)
# Every meta key exists from creation on: see Engine.save.
META_KEYS = {"id": "", "title": "", "cwd": "", "model": "", "effort": "", "mode": "default", "created": 0,
             "updated": 0, "status": "idle", "status_until": None, "conversation_id": None, "turns": 0,
             "queue": [], "goal": None, "last_slot": None, "usage": {"input": 0, "output": 0, "thinking": 0},
             "context": 0, "pending_note": "", "jobs": [], "loop": None}
DENIED_NOTE = ("These commands were denied: headless agy cannot ask for permission, so the turn ended. "
               "Do not retry them or reword them; carry on without them: {cmds}")
AUTO_CONTINUES = 2  # follow-ups after denials per user message, so a stuck agent cannot loop forever
CONTINUE_AFTER_QUOTA = ("Continue exactly where you stopped. Your previous attempt was cut off by an "
                        "account quota limit; do not redo work that is already finished.")


def hhmm(t):
    return datetime.fromtimestamp(t).strftime("%H:%M") if t else "?"


def goal_prompt(g):
    return (f"Objective: {g['objective']}\n\n"
            "Work autonomously until the objective is fully achieved. Do not stop to ask for "
            "confirmation; make reasonable decisions yourself and keep going.\n"
            f"When, and only when, the objective is fully achieved and verified, end your final "
            f"message with the line {DONE_MARK}.")


def parse_verdict(text):
    """{done, reason, missing} from a verifier reply that ends with a VERDICT line; None without one."""
    hits = list(VERDICT_RE.finditer(text or ""))
    if not hits:
        return None
    m = hits[-1]
    body = text[:m.start()].strip()
    done = m.group(1).upper() == "DONE"
    missing = [] if done else [ln.strip()[2:] for ln in body.splitlines() if ln.strip().startswith(("- ", "* "))]
    reason = body[-1500:] or ("Nothing missing." if done else "No reasons given.")
    return {"done": done, "reason": reason, "missing": missing[:20]}


SHELL_TIMEOUT = 600      # seconds a `!command` may run
SHELL_KEEP = 50_000      # characters of its output kept in the transcript
SHELL_NOTE = 6_000       # characters of it passed on to agy
FILE_VIEW_MAX = 2_000_000
# Long commands run as agy-loop background jobs, so a turn never sits waiting on training.
JOBS_HINT = ("(agy-loop) For a command that runs longer than about two minutes (training, long benchmarks, "
             "big test suites), do not run it and wait. End your reply with a line\n"
             "RUN_IN_BACKGROUND: <shell command>\n"
             "and optionally CHECK_EVERY: <minutes>m on the next line for progress check-ins. agy-loop runs it "
             "in this folder, logs its output and sends you the log when it exits.")
BG_LINE = re.compile(r"^[ \t>*_]*RUN_IN_BACKGROUND:[*_]*[ \t]*`*(.+?)`*[ \t]*$", re.M)
EVERY_LINE = re.compile(r"^[ \t>*_]*CHECK_EVERY:[*_]*[ \t]*(\d+(?:\.\d+)?)[ \t]*([smh]?)", re.M | re.I)
JOB_TAIL = 4_000         # characters of a job's log passed to agy
LOOP_MIN = 60            # seconds between /loop prompts, at least


def parse_interval(text, unit="m"):
    """'90s', '10m', '1.5h', '15' (in `unit`) as seconds; 0 if it does not parse."""
    m = re.fullmatch(r"\s*(\d+(?:\.\d+)?)\s*([smh]?)\s*", str(text or ""), re.I)
    if not m:
        return 0
    return int(float(m.group(1)) * {"s": 1, "m": 60, "h": 3600}[(m.group(2) or unit).lower()])


def pid_alive(pid):
    if os.name == "nt":
        out = subprocess.run(["tasklist", "/FI", f"PID eq {pid}", "/NH"], capture_output=True, text=True).stdout
        return str(pid) in out
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def took(secs):
    secs = int(secs)
    return f"{secs // 3600}h{secs // 60 % 60:02d}m" if secs >= 3600 else f"{secs // 60}m{secs % 60:02d}s"


def tail(path, n=JOB_TAIL):
    try:
        with open(path, "rb") as f:
            f.seek(max(0, os.path.getsize(path) - n * 2))
            return ANSI.sub("", f.read().decode("utf-8", "replace"))[-n:]
    except OSError:
        return ""
ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")
MENTION = re.compile(r"(?<![\w`])@([\w./~-]+[\w/])")


def expand_mentions(text, cwd):
    """`@path` mentions that name real files or folders get their absolute paths listed for agy."""
    found = []
    for m in MENTION.finditer(text):
        p = os.path.normpath(os.path.join(cwd, os.path.expanduser(m.group(1))))
        if os.path.exists(p) and p not in found:
            found.append(p)
    if not found:
        return text
    return text + "\n\nReferenced " + ("files" if len(found) > 1 else "file") + ":\n" + "\n".join(f"- {p}" for p in found)


def _read(path):
    """File text for diffing; '' if missing, None if too big or binary."""
    try:
        if os.path.getsize(path) > 1_000_000:
            return None
        with open(path, encoding="utf-8") as f:
            return f.read()
    except FileNotFoundError:
        return ""
    except (OSError, UnicodeDecodeError):
        return None


def _born_after(path, t):
    try:
        st = os.stat(path)
    except OSError:
        return False
    return getattr(st, "st_birthtime", st.st_ctime) >= t - 1


def _diff(path, before, after, cwd):
    if before is None or after is None:
        return ""
    rel = os.path.relpath(path, cwd) if path.startswith(cwd) else path
    lines = list(difflib.unified_diff(before.splitlines(keepends=True), after.splitlines(keepends=True),
                                      fromfile="a/" + rel, tofile="b/" + rel))
    return "".join(lines[:600])


class Session:
    def __init__(self, meta):
        self.meta = meta
        self.lock = threading.Lock()   # thread start / queue
        self.io = threading.Lock()     # events.jsonl appends
        self.stop = threading.Event()
        self.proc = None
        self.thread = None
        self.files = {}  # path -> last content seen, a fallback "before" for diffs
        self.thinking_due = []  # (turn, step, tokens) replies whose thinking was not in agy's db yet
        self.turn_started = 0

    @property
    def dir(self):
        return os.path.join(a.base(), "sessions", self.meta["id"])

    @property
    def busy(self):
        return bool(self.thread and self.thread.is_alive())


class Engine:
    def __init__(self):
        self.subs = []
        self.sessions = {}
        root = os.path.join(a.base(), "sessions")
        for sid in os.listdir(root) if os.path.isdir(root) else []:
            try:
                with open(os.path.join(root, sid, "meta.json")) as f:
                    meta = json.load(f)
            except (OSError, ValueError):
                continue
            meta = {**META_KEYS, **meta, "status": "idle"}  # nothing runs after a restart
            meta["usage"] = {"input": 0, "output": 0, "thinking": 0, **meta["usage"]}
            self.sessions[sid] = Session(meta)
        for s in self.sessions.values():  # background jobs outlive agy-loop: keep watching them
            for job in s.meta["jobs"]:
                if not job["ended"]:
                    threading.Thread(target=self._watch_job, args=(s, job), daemon=True).start()
        threading.Thread(target=self._scheduler, daemon=True).start()

    # --- pub/sub and storage ----------------------------------------------

    def subscribe(self):
        q = queue.Queue(maxsize=20000)
        self.subs.append(q)
        return q

    def unsubscribe(self, q):
        if q in self.subs:
            self.subs.remove(q)

    def publish(self, sid, ev, persist=False):
        ev.setdefault("ts", time.time())
        s = self.sessions.get(sid)
        if persist and s:
            with s.io:
                with open(os.path.join(s.dir, "events.jsonl"), "a") as f:
                    f.write(json.dumps(ev) + "\n")
        for q in list(self.subs):
            try:
                q.put_nowait({"sid": sid, **ev})
            except queue.Full:
                pass

    def notice(self, s, kind, text, **data):
        self.publish(s.meta["id"], {"t": "notice", "kind": kind, "text": text, "data": data}, persist=True)

    def save(self, s):
        s.meta["updated"] = time.time()
        os.makedirs(s.dir, exist_ok=True)
        # One snapshot for disk and subscribers. meta's keys are fixed at creation, so another
        # thread changing a value cannot break the serialization half-way.
        text = json.dumps(s.meta)
        a.write_atomic(os.path.join(s.dir, "meta.json"), text)
        self.publish(s.meta["id"], {"t": "meta", "meta": json.loads(text)})

    def status(self, s, status, **extra):
        s.meta.update(status=status, status_until=extra.get("until"))
        self.save(s)

    def events(self, sid):
        try:
            with open(os.path.join(self.sessions[sid].dir, "events.jsonl")) as f:
                return [json.loads(line) for line in f if line.strip()]
        except OSError:
            return []

    # --- session lifecycle ----------------------------------------------

    def create(self, cwd, model, effort="", mode="default", title=""):
        cwd = os.path.abspath(os.path.expanduser(cwd))
        if not os.path.isdir(cwd):
            raise ValueError(f"not a directory: {cwd}")
        if mode not in MODES:
            raise ValueError(f"unknown mode: {mode}")
        sid = uuid.uuid4().hex[:12]
        now = time.time()
        s = Session({**META_KEYS, "id": sid, "title": title, "cwd": cwd, "model": model, "effort": effort,
                     "mode": mode, "created": now, "updated": now, "queue": [],
                     "usage": {"input": 0, "output": 0, "thinking": 0}})
        self.sessions[sid] = s
        self.save(s)
        return s.meta

    def update(self, sid, fields):
        s = self.sessions[sid]
        for k in ("title", "model", "effort", "mode"):
            if k in fields:
                if k == "mode" and fields[k] not in MODES:
                    raise ValueError(f"unknown mode: {fields[k]}")
                s.meta[k] = fields[k]
        self.save(s)
        return s.meta

    def delete(self, sid):
        self.stop(sid)
        s = self.sessions.pop(sid)
        shutil.rmtree(s.dir, ignore_errors=True)
        self.publish(sid, {"t": "deleted"})

    def attachments_dir(self, sid):
        return os.path.join(self.sessions[sid].dir, "attachments")

    def attach(self, sid, filename, data):
        """Keep an uploaded file with the session; agy sees the folder through --add-dir."""
        name = re.sub(r"[^\w.\- ]+", "_", os.path.basename(filename or "file")).strip(" .") or "file"
        d = self.attachments_dir(sid)
        os.makedirs(d, exist_ok=True)
        stem, ext = os.path.splitext(name)
        path, n = os.path.join(d, name), 1
        while os.path.exists(path):
            n += 1
            path = os.path.join(d, f"{stem}-{n}{ext}")
        with open(path, "wb") as f:
            f.write(data)
        return {"name": os.path.basename(path), "path": path, "size": len(data),
                "image": ext.lower() in (".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".heic")}

    def send(self, sid, text, kind="user", attachments=None):
        s = self.sessions[sid]
        d = os.path.realpath(self.attachments_dir(sid))
        attachments = [p for p in (attachments or []) if os.path.realpath(p).startswith(d + os.sep) and os.path.isfile(p)]
        with s.lock:
            if s.busy:
                s.meta["queue"].append({"text": text, "attachments": attachments, "kind": kind}
                                       if attachments or kind != "user" else text)
                self.save(s)
                return "queued"
            s.stop.clear()
            s.thread = threading.Thread(target=self._loop, args=(s, text, kind, attachments), daemon=True)
            s.thread.start()
            return "started"

    def stop(self, sid):
        s = self.sessions[sid]
        s.stop.set()
        s.meta["queue"] = []
        if s.proc and s.proc.poll() is None:
            s.proc.terminate()
        g = s.meta.get("goal")
        if g and g["state"] == "active":
            g["state"] = "paused"
        self.save(s)

    def set_goal(self, sid, objective, check="", verifier=True, verifier_model="", max_rounds=20, start=True):
        s = self.sessions[sid]
        s.meta["goal"] = {"objective": objective.strip(), "check": check.strip(), "verifier": bool(verifier),
                          "verifier_model": verifier_model, "max_rounds": int(max_rounds), "round": 0,
                          "state": "active"}
        self.save(s)
        self.notice(s, "goal", f"Goal set: {objective.strip()}")
        if start:
            self.send(sid, goal_prompt(s.meta["goal"]), kind="goal")
        return s.meta

    def resume_goal(self, sid):
        s = self.sessions[sid]
        g = s.meta.get("goal")
        if not g:
            raise ValueError("no goal")
        g.update(state="active", round=0)
        self.save(s)
        self.send(sid, "Continue working toward the objective: " + g["objective"]
                  + f"\nEnd with {DONE_MARK} only when it is fully achieved.", kind="nudge")

    def clear_goal(self, sid):
        s = self.sessions[sid]
        s.meta["goal"] = None
        self.save(s)

    # --- the loop ---------------------------------------------------------

    def _loop(self, s, text, kind, attachments=None):
        m, errors, continues = s.meta, 0, 0
        try:
            while True:
                if not m["title"] and kind == "user":
                    m["title"] = (text.strip().splitlines() or ["(attachment)"])[0][:60]
                files = [{"name": os.path.basename(p), "path": p,
                          "image": os.path.splitext(p)[1].lower() in (".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".heic")}
                         for p in attachments or []]
                self.publish(m["id"], {"t": "user", "text": text, "kind": kind, "attachments": files}, persist=True)
                prompt = expand_mentions(text, m["cwd"])
                if files:
                    prompt += ("\n\nAttached files (open each with your file viewer; images are visible to you):\n"
                               + "\n".join(f"- {f['path']}" for f in files))
                attachments = None
                if m.get("pending_note"):  # a rewind or compact happened since the last turn
                    prompt, m["pending_note"] = m["pending_note"] + "\n\n" + prompt, ""
                if not m.get("conversation_id"):  # the request itself stays last
                    prompt = JOBS_HINT + "\n\n" + prompt
                ok, reply, denied = self._turn(s, prompt)
                if ok:
                    self._jobs_from_reply(s, reply)
                waiting = [j for j in m["jobs"] if not j["ended"] and j["notify"]]
                text = None
                note = DENIED_NOTE.format(cmds=", ".join(f"`{c}`" for c in denied)) if denied else ""
                if kind == "user":
                    continues = 0
                g = m.get("goal")
                if g and g["state"] == "active" and not s.stop.is_set():
                    if ok and waiting and DONE_MARK not in reply:
                        self.notice(s, "goal", f"Waiting for background job `{waiting[0]['cmd']}`"
                                               + (f" and {len(waiting) - 1} more" if len(waiting) > 1 else "")
                                               + "; the goal continues when it finishes.")
                    elif ok:
                        errors = 0
                        text, kind = self._goal_next(s, reply), "nudge"
                        if text and note:
                            text = note + "\n\n" + text
                    elif errors < 3:  # transient failure in a long run: back off and push on
                        errors += 1
                        self.notice(s, "goal", f"The turn failed; retrying in {30 * errors}s ({errors}/3).")
                        if not s.stop.wait(30 * errors):
                            text, kind = "Continue working toward the objective: " + g["objective"], "nudge"
                elif ok and note and continues < AUTO_CONTINUES and not s.stop.is_set():
                    continues += 1
                    text, kind = note + "\n\nContinue the task.", "nudge"
                with s.lock:  # decide under the lock, so a message sent right now is never stranded
                    if text is None and m["queue"] and not s.stop.is_set():
                        item = m["queue"].pop(0)
                        if isinstance(item, dict):
                            text, attachments, kind = item["text"], item.get("attachments"), item.get("kind", "user")
                        else:
                            text, attachments, kind = item, None, "user"
                    if text is None or s.stop.is_set():
                        s.thread = None
                        self.status(s, "idle")
                        return
        except Exception as e:  # never leave a session stuck in "running"
            self.notice(s, "error", f"internal error: {e!r}")
            with s.lock:
                s.thread = None
                self.status(s, "idle")

    def _turn(self, s, prompt):
        """Run one turn, failing over between accounts. Returns (ok, final_text)."""
        m = s.meta
        if not a.slots():
            self.notice(s, "noaccount", "No Google account is signed in to agy-loop yet. Add one, then send again.")
            return False, "", []
        bucket = a.bucket_of(["--model", m["model"]])
        pool = a.BUCKETS[bucket]
        failovers = 0
        while failovers <= 2 * len(a.slots()) + 1:  # bounded: every account walled twice is enough
            self.status(s, "waiting")
            slot, fd = a.acquire(bucket, stop=s.stop)
            if s.stop.is_set():
                return False, "", []
            if slot is None:
                nxt = a.next_reset(bucket)
                if not (m.get("goal") or {}).get("state") == "active":
                    self.notice(s, "wall", f"Every account is out of {pool} quota. Next reset {hhmm(nxt)}. "
                                           "Send again later, or use goal mode to wait automatically.")
                    return False, "", []
                self.notice(s, "wall", f"Every account is out of {pool} quota. Sleeping until {hhmm(nxt)}, "
                                       "then the goal continues.")
                self.status(s, "sleeping", until=nxt)
                if s.stop.wait(max(5, nxt - time.time() + 30)):
                    return False, "", []
                continue
            try:
                r = self._run_agy(s, slot, prompt)
                if r["quota"]:
                    a.wall_after_quota_error(slot, bucket)
            finally:
                os.close(fd)
            if r["quota"]:
                self.notice(s, "handoff", f"{slot} ran out of {pool} quota (back {hhmm(a.walled_until(slot, bucket))}). "
                                          "Passing the turn to the next account.", slot=slot)
                prompt = CONTINUE_AFTER_QUOTA
                failovers += 1
                continue
            if r["conv_lost"]:
                self.notice(s, "handoff", "The conversation could not be resumed on this account; "
                                          "starting a fresh one with a handoff summary.")
                m["conversation_id"] = None
                prompt = self._handoff(s, prompt)
                failovers += 1
                continue
            return r["ok"], r["text"], r["denied"]
        return False, "", []

    def _run_agy(self, s, slot, prompt):
        m, sid = s.meta, s.meta["id"]
        resumed = bool(m.get("conversation_id"))
        args = ["-p", prompt, "--output-format", "stream-json", "--model", m["model"],
                *MODES[m["mode"]],  # effort is part of the model id (gemini-3.8-flash-high/-medium/-low)
                *(["--conversation", m["conversation_id"]] if resumed else []),
                *[x for d in (self.attachments_dir(sid), self.jobs_dir(sid)) if os.path.isdir(d) for x in ("--add-dir", d)]]
        a.sync(a.slot_home(slot))
        m["turns"] += 1
        turn, m["last_slot"] = m["turns"], slot
        self.status(s, "running")
        self.publish(sid, {"t": "turn_start", "turn": turn, "slot": slot, "model": m["model"]}, persist=True)
        t0 = s.turn_started = time.time()
        cmd, env = a.agy_cmd(a.slot_home(slot), args, {**os.environ, "AGYLOOP_SLOT": slot})
        proc = subprocess.Popen(cmd, cwd=m["cwd"], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                text=True, bufsize=1)
        s.proc = proc
        err = []
        reader = threading.Thread(target=lambda: err.append(proc.stderr.read()), daemon=True)
        reader.start()
        texts, before, tail, result, denied = {}, {}, [], {}, []
        last = None                       # (step_type, state) of this turn's latest step
        usage = {"input_tokens": 0, "output_tokens": 0, "thinking_tokens": 0, "total_tokens": 0}
        context = 0  # the largest prompt this turn sent: history + tools + cached part
        for line in proc.stdout:
            try:
                d = json.loads(line)
            except ValueError:
                continue
            ev = d.get("event")
            if ev == "init":
                if d.get("conversation_id") and d["conversation_id"] != m.get("conversation_id"):
                    m["conversation_id"] = d["conversation_id"]
                    self.save(s)
            elif ev == "step_update":
                su = d.get("step_update") or {}
                self._step(s, turn, su, texts, before, denied)
                last = (su.get("step_type"), su.get("state"))
                step_usage = (su.get("usage") or {}) if su.get("state") == "DONE" else {}
                for k in usage:  # per step: agy's result totals the whole conversation
                    usage[k] += step_usage.get(k, 0)
                context = max(context, step_usage.get("input_tokens", 0) + step_usage.get("cache_read_tokens", 0))
            else:
                tail.append(line)
                if ev == "result":
                    result = d.get("result") or {}
        proc.wait()
        reader.join(timeout=5)
        s.proc = None
        due, s.thinking_due = s.thinking_due, []
        for t, step, tokens in due:
            self._thinking(s, t, step, tokens)
        err = "".join(err).strip()
        stopped = s.stop.is_set()
        # In a resumed conversation agy's result keeps the last error of the conversation, e.g. a
        # quota error from the account that failed before this one. A turn that ends with a
        # finished reply worked, whatever that result says.
        replied = last == ("agent_response", "DONE")
        quota = not stopped and not replied and a.is_quota_error(proc.returncode, "".join(tail), err)
        ok = not stopped and not quota and (replied or (proc.returncode == 0 and result.get("status") == "SUCCESS"))
        conv_lost = (resumed and not ok and not quota and not stopped
                     and bool(re.search(r"conversation", err + json.dumps(result), re.I)))
        m["usage"]["input"] += usage.get("input_tokens", 0)
        m["usage"]["output"] += usage.get("output_tokens", 0)
        m["usage"]["thinking"] = m["usage"].get("thinking", 0) + usage.get("thinking_tokens", 0)
        if context:
            m["context"] = context
        usage["context"] = context
        if usage["total_tokens"] or usage["input_tokens"]:  # per-account ledger for the accounts panel
            with open(a.state("usage.jsonl"), "a") as f:
                f.write(json.dumps({"t": int(time.time()), "slot": slot, "model": m["model"], "session": sid,
                                    "input": usage["input_tokens"], "output": usage["output_tokens"],
                                    "thinking": usage["thinking_tokens"]}) + "\n")
        if result.get("denied_actions") or denied:
            self.notice(s, "denied", "agy denied a tool call that headless mode cannot ask about.",
                        actions=result.get("denied_actions") or [], commands=denied)
        elif err and not ok and not quota and not stopped and not conv_lost:
            self.notice(s, "error", err[-4000:])
        status = ("stopped" if stopped else "quota" if quota else "SUCCESS" if ok else
                  result.get("status") or f"exit {proc.returncode}")
        self.publish(sid, {"t": "turn_end", "turn": turn, "slot": slot, "status": status, "usage": usage,
                           "secs": round(time.time() - t0, 1)}, persist=True)
        self.save(s)
        final = (texts[max(texts)] if texts else "") or result.get("response") or ""
        return {"ok": ok, "quota": quota, "conv_lost": conv_lost, "text": final, "denied": [c for c in denied if c]}

    def _step(self, s, turn, su, texts, before, denied):
        sid, step, state = s.meta["id"], su.get("step_index"), su.get("state")
        kind = su.get("step_type")
        if kind == "agent_response":
            delta = su.get("text_delta") or ""
            if delta:
                texts[step] = texts.get(step, "") + delta
                self.publish(sid, {"t": "delta", "turn": turn, "step": step, "text": delta})
            if state != "ACTIVE":
                tokens = (su.get("usage") or {}).get("thinking_tokens", 0)
                if not self._thinking(s, turn, step, tokens) and tokens:
                    s.thinking_due.append((turn, step, tokens))
            if state != "ACTIVE" and texts.get(step):
                self.publish(sid, {"t": "assistant", "turn": turn, "step": step, "text": texts[step]}, persist=True)
        elif kind == "tool":
            info = su.get("tool_info") or {}
            name = info.get("name") or su.get("tool_name") or "tool"
            params = info.get("parameters") or {}
            path = params.get("TargetFile") if name in EDIT_TOOLS else None
            ev = {"t": "tool", "turn": turn, "step": step, "name": name, "params": params}
            if state == "ACTIVE":
                if path and step not in before:
                    before[step] = _read(path)
                self.publish(sid, {**ev, "state": "running"})
                return
            ev.update(state="error" if state == "ERROR" else "done", secs=su.get("duration_seconds"),
                      output=ANSI.sub("", str(info.get("output") or ""))[:OUT_CAP])
            if name == "view_file" and params.get("AbsolutePath"):
                self._remember(s, params["AbsolutePath"], _read(params["AbsolutePath"]))
            if info.get("error"):
                ev["error"] = str(info["error"].get("message") or info["error"])[:OUT_CAP]
                if name == "run_command" and "denied permission" in ev["error"]:
                    denied.append(params.get("CommandLine", ""))
            if path:
                b, after = before.get(step), _read(path)
                if (b is None or b == after) and path in s.files:  # we read "before" too late
                    b = s.files[path]
                elif b == after and name == "write_to_file" and _born_after(path, s.turn_started):
                    b = ""  # created this turn, faster than our snapshot: all of it is new
                ev["diff"] = _diff(path, b, after, s.meta["cwd"])
                self._remember(s, path, after)
                self._checkpoint(s, turn, path, b, after)
            self.publish(sid, ev, persist=True)

    def _thinking(self, s, turn, step, tokens=0):
        """Publish a reply's thinking, read from agy's conversation db. True if found."""
        conv_dir = os.path.join(a.stack_home(), ".gemini", "antigravity-cli", "conversations")
        text = convdb.thinking(conv_dir, s.meta.get("conversation_id"), step)
        if text:
            self.publish(s.meta["id"], {"t": "thinking", "turn": turn, "step": step, "text": text, "tokens": tokens},
                         persist=True)
        return bool(text)

    @staticmethod
    def _checkpoint(s, turn, path, before, after):
        """Remember a file's content before this turn first touched it, for rewinds."""
        if before is None or after is None:
            return
        p = os.path.join(s.dir, "checkpoints", f"{turn}.json")
        os.makedirs(os.path.dirname(p), exist_ok=True)
        try:
            with open(p) as f:
                cp = json.load(f)
        except (OSError, ValueError):
            cp = {}
        cp.setdefault(path, {"before": before})["after"] = after
        a.write_atomic(p, json.dumps(cp))

    def rewind(self, sid, turn):
        """Put every file the agent changed in `turn` and later back as it was before `turn`."""
        s = self.sessions[sid]
        if s.busy:
            raise ValueError("stop the running turn first")
        d = os.path.join(s.dir, "checkpoints")
        turns = sorted(int(n[:-5]) for n in (os.listdir(d) if os.path.isdir(d) else []) if n.endswith(".json"))
        restore, latest = {}, {}
        for t in turns:
            if t < int(turn):
                continue
            with open(os.path.join(d, f"{t}.json")) as f:
                for path, c in json.load(f).items():
                    restore.setdefault(path, c["before"])
                    latest[path] = c["after"]
        if not restore:
            raise ValueError(f"no file changes from turn {turn} on")
        outside = []
        for path, content in restore.items():
            if _read(path) != latest[path]:
                outside.append(path)  # changed after the agent's last edit: still restored, but said so
            if content == "":
                if os.path.exists(path):
                    os.remove(path)
            else:
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "w", encoding="utf-8") as f:
                    f.write(content)
            s.files.pop(path, None)
        for t in turns:
            if t >= int(turn):
                os.remove(os.path.join(d, f"{t}.json"))
        rel = sorted(os.path.relpath(p, s.meta["cwd"]) for p in restore)
        s.meta["pending_note"] = (f"Files were rewound to how they were before turn {turn}: {', '.join(rel)}. "
                                  "Re-read them before editing.")
        self.save(s)
        self.notice(s, "rewind", f"Rewound {len(rel)} file{'s' if len(rel) > 1 else ''} to before turn {turn}.",
                    files=rel, outside=[os.path.relpath(p, s.meta["cwd"]) for p in outside], turn=int(turn))
        return rel

    def clear(self, sid):
        """Start a fresh agy conversation; the transcript stays."""
        s = self.sessions[sid]
        if s.busy:
            raise ValueError("stop the running turn first")
        s.meta.update(conversation_id=None, context=0, pending_note="")
        self.save(s)
        self.notice(s, "info", "New conversation: agy starts fresh from the next message.")

    def compact(self, sid):
        """A fresh agy conversation that starts from a summary of this one."""
        s = self.sessions[sid]
        if s.busy:
            raise ValueError("stop the running turn first")
        s.meta.update(conversation_id=None, context=0, pending_note=self._handoff(s, "(the next message follows)"))
        self.save(s)
        self.notice(s, "info", "Compacted: the next message starts a fresh conversation from a summary of this one.")

    # --- background jobs and /loop ----------------------------------------

    def jobs_dir(self, sid):
        return os.path.join(self.sessions[sid].dir, "jobs")

    def job_start(self, sid, cmd, every=0, notify=True, by="user"):
        """Run `cmd` detached in the session folder, logging to a file. When it exits (and every
        `every` seconds while it runs) agy gets the end of the log as the next message."""
        s = self.sessions[sid]
        jid = uuid.uuid4().hex[:8]
        os.makedirs(self.jobs_dir(sid), exist_ok=True)
        log = os.path.join(self.jobs_dir(sid), f"{jid}.log")
        # Its own process group, so stopping agy-loop leaves training running and Stop kills it all.
        detach = {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP} if os.name == "nt" else {"start_new_session": True}
        with open(log, "wb") as out:
            proc = subprocess.Popen(cmd, shell=True, cwd=s.meta["cwd"], stdin=subprocess.DEVNULL, stdout=out,
                                    stderr=subprocess.STDOUT, env={**os.environ, "NO_COLOR": "1", "PYTHONUNBUFFERED": "1"},
                                    **detach)
        job = {"id": jid, "cmd": cmd, "pid": proc.pid, "log": log, "started": time.time(), "ended": None,
               "code": None, "every": int(every or 0), "notify": bool(notify), "by": by, "stopped": False}
        with s.lock:
            s.meta["jobs"] = (s.meta["jobs"] + [job])[-20:]
            self.save(s)
        self.publish(sid, {"t": "job", "phase": "started", "job": dict(job)}, persist=True)
        threading.Thread(target=self._watch_job, args=(s, job, proc), daemon=True).start()
        return job

    def _jobs_from_reply(self, s, reply):
        every = EVERY_LINE.search(reply or "")
        secs = parse_interval(every.group(1) + (every.group(2) or "m")) if every else 0
        for cmd in BG_LINE.findall(reply or ""):
            if cmd.strip() and not any(j["cmd"] == cmd.strip() and not j["ended"] for j in s.meta["jobs"]):
                self.job_start(s.meta["id"], cmd.strip(), every=secs, by="agent")

    def _watch_job(self, s, job, proc=None):
        sid, last = s.meta["id"], time.time()
        while True:
            if proc is not None:
                try:
                    code = proc.wait(timeout=5)
                    break
                except subprocess.TimeoutExpired:
                    pass
            else:  # adopted after a restart: not our child, so poll the pid
                if not pid_alive(job["pid"]):
                    code = None
                    break
                time.sleep(5)
            if job["every"] and time.time() - last >= job["every"]:
                last = time.time()
                if job["notify"] and not s.busy and not s.stop.is_set():
                    self.send(sid, f"Background job `{job['cmd']}` is still running ({took(last - job['started'])}, "
                                   f"pid {job['pid']}). End of its log:\n```\n{tail(job['log'])}\n```\nIf it is going "
                                   f"wrong, stop it (`kill {job['pid']}`) and fix the cause; otherwise reply in one line "
                                   "and do not wait for it.", kind="job")
        with s.lock:
            job.update(ended=time.time(), code=code)
            self.save(s)
        self.publish(sid, {"t": "job", "phase": "ended", "job": dict(job)}, persist=True)
        if job["notify"] and not job["stopped"] and not s.stop.is_set():
            status = f"exit {code}" if code is not None else "finished"
            self.send(sid, f"Background job `{job['cmd']}` {status} after {took(job['ended'] - job['started'])}. "
                           f"End of its log ({job['log']}):\n```\n{tail(job['log'])}\n```\n"
                           "Analyze the result and continue.", kind="job")

    def job_stop(self, sid, jid):
        s = self.sessions[sid]
        job = next(j for j in s.meta["jobs"] if j["id"] == jid)
        if job["ended"]:
            return job
        job["stopped"] = True
        try:
            if os.name == "nt":
                subprocess.run(["taskkill", "/T", "/F", "/PID", str(job["pid"])], capture_output=True)
            else:
                os.killpg(job["pid"], signal.SIGTERM)
        except OSError:
            pass
        self.save(s)
        return job

    def job_log(self, sid, jid):
        job = next(j for j in self.sessions[sid].meta["jobs"] if j["id"] == jid)
        return {**job, "text": tail(job["log"], 200_000)}

    def set_loop(self, sid, every, prompt):
        """Send `prompt` now and then every `every` seconds while the session is idle (Stop pauses it)."""
        s = self.sessions[sid]
        if every and not str(prompt).strip():
            raise ValueError("what should the loop send? /loop 10m <prompt>")
        s.meta["loop"] = {"every": max(LOOP_MIN, int(every)), "prompt": prompt.strip(), "next": time.time()} if every else None
        self.save(s)
        if every:
            s.stop.clear()

    def _scheduler(self):
        while True:
            time.sleep(5)
            for sid, s in list(self.sessions.items()):
                lp = s.meta.get("loop")
                if lp and time.time() >= lp["next"] and not s.busy and not s.stop.is_set():
                    s.meta["loop"] = {**lp, "next": time.time() + lp["every"]}
                    self.save(s)
                    self.send(sid, lp["prompt"], kind="loop")

    def shell(self, sid, cmd, timeout=SHELL_TIMEOUT):
        """The user's own `!command`, run in the project folder. agy sees it with the next message."""
        s = self.sessions[sid]
        t0 = time.time()
        try:
            p = subprocess.run(cmd, shell=True, cwd=s.meta["cwd"], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, timeout=timeout, env={**os.environ, "NO_COLOR": "1"})
            out, code = p.stdout, p.returncode
        except subprocess.TimeoutExpired as e:
            out, code = (e.stdout or b"") + f"\n(stopped after {timeout}s)".encode(), None
        out = ANSI.sub("", out.decode("utf-8", "replace"))[-SHELL_KEEP:]
        ev = {"t": "shell", "cmd": cmd, "output": out, "code": code, "secs": round(time.time() - t0, 1)}
        self.publish(sid, ev, persist=True)
        tail = out[-SHELL_NOTE:]
        note = (f"The user ran `{cmd}` in the project folder (exit {code if code is not None else 'timeout'}). "
                f"Output{' (last part)' if len(out) > len(tail) else ''}:\n```\n{tail}\n```")
        with s.lock:
            s.meta["pending_note"] = "\n\n".join(x for x in (s.meta["pending_note"], note) if x)
            self.save(s)
        return ev

    def note(self, sid, text):
        """A `#` note: appended to the project's GEMINI.md (or AGENTS.md if that is the one in use),
        which agy loads at the start of every conversation in this folder."""
        cwd = self.sessions[sid].meta["cwd"]
        path = next((os.path.join(cwd, n) for n in ("GEMINI.md", "AGENTS.md") if os.path.isfile(os.path.join(cwd, n))),
                    os.path.join(cwd, "GEMINI.md"))
        with open(path, "a+", encoding="utf-8") as f:
            f.seek(0)
            body = f.read()
            f.write(("" if not body or body.endswith("\n") else "\n") + f"- {text.strip()}\n")
        return {"path": path, "file": os.path.basename(path)}

    def read_file(self, sid, path):
        """A text file inside the session's folder or its attachments, for the GUI's file viewer."""
        s = self.sessions[sid]
        p = os.path.realpath(os.path.join(s.meta["cwd"], os.path.expanduser(path)))
        roots = [os.path.realpath(s.meta["cwd"]), os.path.realpath(self.attachments_dir(sid))]
        if not any(p == r or p.startswith(r + os.sep) for r in roots):
            raise ValueError("only files inside the session folder can be opened here")
        if os.path.getsize(p) > FILE_VIEW_MAX:
            raise ValueError(f"{os.path.basename(p)} is over {FILE_VIEW_MAX // 1_000_000} MB")
        with open(p, "rb") as f:
            data = f.read()
        if b"\0" in data[:8000]:
            raise ValueError(f"{os.path.basename(p)} is not a text file")
        return {"path": p, "rel": os.path.relpath(p, s.meta["cwd"]), "text": data.decode("utf-8", "replace")}

    @staticmethod
    def _remember(s, path, content):
        if content is None:
            return
        s.files.pop(path, None)
        s.files[path] = content
        while len(s.files) > 200:  # ponytail: plain FIFO cap, enough for one session's working set
            s.files.pop(next(iter(s.files)))

    # --- goal mode -----------------------------------------------------------

    def _goal_next(self, s, reply):
        """Next prompt for an active goal, or None when it is met or abandoned."""
        g = s.meta["goal"]
        g["round"] += 1
        self.save(s)
        if g["round"] > g["max_rounds"]:
            g["state"] = "failed"
            self.notice(s, "goal", f"Goal abandoned after {g['max_rounds']} rounds.")
            self.save(s)
            return None
        if DONE_MARK not in reply:
            self.notice(s, "goal", f"The agent stopped before claiming the goal is done. Pushing it on "
                                   f"(round {g['round']}/{g['max_rounds']}).")
            return ("You stopped before the objective was achieved. Keep working toward it:\n"
                    f"{g['objective']}\nEnd with {DONE_MARK} only when it is fully achieved.")
        problems = []
        if g.get("check"):
            self.status(s, "checking")
            try:
                r = subprocess.run(g["check"], shell=True, cwd=s.meta["cwd"], capture_output=True,
                                   text=True, timeout=1800)
                rc, out = r.returncode, (r.stdout + r.stderr)[-3000:]
            except subprocess.TimeoutExpired:
                rc, out = "timeout", ""
            self.notice(s, "check", f"Check `{g['check']}` exited {rc}.", rc=rc, output=out, passed=rc == 0)
            if rc != 0:
                problems.append(f"The check command `{g['check']}` exited {rc}:\n{out}")
        if not problems and g.get("verifier"):
            v = self._verify(s)
            if s.stop.is_set():
                return None
            if v is None:
                problems.append("The independent verifier could not run; re-check your work carefully.")
            elif not v.get("done"):
                missing = "\n".join(f"- {x}" for x in v.get("missing") or [])
                problems.append(f"An independent verifier says the objective is not met: {v.get('reason', '')}\n{missing}")
        if problems:
            return ("The objective is not met yet.\n\n" + "\n\n".join(problems)
                    + f"\n\nFix this and continue. End with {DONE_MARK} only when everything passes.")
        g["state"] = "met"
        self.notice(s, "goal", "Goal met.", met=True)
        self.save(s)
        return None

    def _verify(self, s):
        """Fresh read-only agy call that judges the objective. None if it cannot run."""
        m, g = s.meta, s.meta["goal"]
        self.status(s, "verifying")
        # The verifier's own model first; when its quota is gone on every account (Claude/GPT
        # limits are small), the session's model judges instead, so the goal never stalls on it.
        models = list(dict.fromkeys(x for x in (g.get("verifier_model"), m["model"]) if x))
        for i, model in enumerate(models):
            bucket = a.bucket_of(["--model", model])
            while not s.stop.is_set():
                slot, fd = a.acquire(bucket, stop=s.stop)
                if slot is None:
                    break
                reply = self._verifier_call(s, slot, fd, model, bucket)
                if reply is not None:
                    return self._verdict(s, reply, slot, model)
            if i + 1 < len(models) and not s.stop.is_set():
                self.notice(s, "info", f"No account has {a.BUCKETS[bucket]} quota for the verifier ({model}); "
                                       f"{models[i + 1]} verifies instead.")
        self.notice(s, "verdict", "No account has quota left for the verifier.", done=None)
        return None

    def _verifier_call(self, s, slot, fd, model, bucket):
        """One read-only agy call; its reply text, or None after a quota error (the slot gets walled)."""
        m, g = s.meta, s.meta["goal"]
        # Headless agy ends the turn on any denied tool, so the verifier sticks to viewing files.
        prompt = ("You are a strict, independent verifier. Another agent claims this objective is complete:\n\n"
                  f"{g['objective']}\n\nThe work is in the current directory. Inspect it yourself with file "
                  "viewing and listing tools only: shell commands and edits are blocked here and would end your "
                  "turn. Do not modify anything, do not plan fixes, and do not ask questions: only judge. Say in one "
                  "or two sentences what you checked, list anything missing or broken as '- ' bullets, then "
                  "end with exactly one final line: "
                  "VERDICT: DONE if the objective is fully achieved, or VERDICT: NOT_DONE otherwise.")
        try:
            a.sync(a.slot_home(slot))
            cmd, env = a.agy_cmd(a.slot_home(slot), ["-p", prompt, "--output-format", "json", "--model", model],
                                 {**os.environ, "AGYLOOP_SLOT": slot})
            proc = subprocess.Popen(cmd, cwd=m["cwd"], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            s.proc = proc
            out, err = proc.communicate()
            s.proc = None
            if a.is_quota_error(proc.returncode, out, err):
                a.wall_after_quota_error(slot, bucket)
                self.notice(s, "info", f"The verifier hit the {a.BUCKETS[bucket]} quota on {slot}.")
                return None
            return json.loads(out)["response"]
        except (ValueError, KeyError, TypeError):
            return ""
        finally:
            os.close(fd)

    def _verdict(self, s, reply, slot, model):
        v = parse_verdict(reply)
        if v is None:
            self.notice(s, "verdict", "The verifier gave no verdict. Its reply: " + (reply[-600:] or "(empty)"), done=None)
            return None
        self.notice(s, "verdict", v.get("reason", ""), done=bool(v.get("done")), missing=v.get("missing") or [],
                    slot=slot, model=model)
        return v

    def _handoff(self, s, prompt):
        """Prompt for a fresh conversation that picks up a session's work."""
        evs = self.events(s.meta["id"])[-300:]
        users = [e["text"] for e in evs if e["t"] == "user" and e.get("kind") == "user"][-5:]
        says = [e["text"][-1500:] for e in evs if e["t"] == "assistant"][-3:]
        tools = [f"- {e['name']} {e['params'].get('TargetFile') or e['params'].get('CommandLine') or e['params'].get('AbsolutePath') or ''}"
                 for e in evs if e["t"] == "tool"][-25:]
        g = s.meta.get("goal")
        parts = ["You are taking over work from an earlier agent session whose conversation could not be resumed."]
        if g:
            parts.append("Objective: " + g["objective"])
        parts += ["Recent user requests:\n" + "\n".join(f"- {u[:500]}" for u in users),
                  "Recent tool steps:\n" + "\n".join(tools),
                  "The earlier agent's last messages:\n" + "\n---\n".join(says),
                  "Check the current state of the files yourself, then continue. Current request:\n" + prompt]
        return "\n\n".join(parts)
