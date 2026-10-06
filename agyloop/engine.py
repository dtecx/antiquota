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
import subprocess
import threading
import time
import uuid
from datetime import datetime

from . import accounts as a

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
             "queue": [], "goal": None, "last_slot": None, "usage": {"input": 0, "output": 0}}
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
    return {"done": done, "reason": body[-1500:], "missing": missing[:20]}


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
            self.sessions[sid] = Session(meta)

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
        p = os.path.join(s.dir, "meta.json")
        with open(p + ".tmp", "w") as f:
            f.write(text)
        os.replace(p + ".tmp", p)
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
                     "mode": mode, "created": now, "updated": now, "queue": [], "usage": {"input": 0, "output": 0}})
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

    def send(self, sid, text, kind="user"):
        s = self.sessions[sid]
        with s.lock:
            if s.busy:
                s.meta["queue"].append(text)
                self.save(s)
                return "queued"
            s.stop.clear()
            s.thread = threading.Thread(target=self._loop, args=(s, text, kind), daemon=True)
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

    def _loop(self, s, text, kind):
        m, errors = s.meta, 0
        try:
            while True:
                if not m["title"] and kind == "user":
                    m["title"] = text.strip().splitlines()[0][:60]
                self.publish(m["id"], {"t": "user", "text": text, "kind": kind}, persist=True)
                ok, reply = self._turn(s, text)
                text = None
                g = m.get("goal")
                if g and g["state"] == "active" and not s.stop.is_set():
                    if ok:
                        errors = 0
                        text, kind = self._goal_next(s, reply), "nudge"
                    elif errors < 3:  # transient failure in a long run: back off and push on
                        errors += 1
                        self.notice(s, "goal", f"The turn failed; retrying in {30 * errors}s ({errors}/3).")
                        if not s.stop.wait(30 * errors):
                            text, kind = "Continue working toward the objective: " + g["objective"], "nudge"
                with s.lock:  # decide under the lock, so a message sent right now is never stranded
                    if text is None and m["queue"] and not s.stop.is_set():
                        text, kind = m["queue"].pop(0), "user"
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
        bucket = a.bucket_of(["--model", m["model"]])
        pool = a.BUCKETS[bucket]
        failovers = 0
        while failovers <= 2 * len(a.slots()) + 1:  # bounded: every account walled twice is enough
            self.status(s, "waiting")
            slot, fd = a.acquire(bucket, stop=s.stop)
            if s.stop.is_set():
                return False, ""
            if slot is None:
                nxt = a.next_reset(bucket)
                if not (m.get("goal") or {}).get("state") == "active":
                    self.notice(s, "wall", f"Every account is out of {pool} quota. Next reset {hhmm(nxt)}. "
                                           "Send again later, or use goal mode to wait automatically.")
                    return False, ""
                self.notice(s, "wall", f"Every account is out of {pool} quota. Sleeping until {hhmm(nxt)}, "
                                       "then the goal continues.")
                self.status(s, "sleeping", until=nxt)
                if s.stop.wait(max(5, nxt - time.time() + 30)):
                    return False, ""
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
            return r["ok"], r["text"]
        return False, ""

    def _run_agy(self, s, slot, prompt):
        m, sid = s.meta, s.meta["id"]
        resumed = bool(m.get("conversation_id"))
        cmd = [a.agy_bin(), "-p", prompt, "--output-format", "stream-json", "--model", m["model"],
               *(["--effort", m["effort"]] if m.get("effort") else []), *MODES[m["mode"]],
               *(["--conversation", m["conversation_id"]] if resumed else [])]
        a.sync(a.slot_home(slot))
        m["turns"] += 1
        turn, m["last_slot"] = m["turns"], slot
        self.status(s, "running")
        self.publish(sid, {"t": "turn_start", "turn": turn, "slot": slot, "model": m["model"]}, persist=True)
        t0 = time.time()
        proc = subprocess.Popen(cmd, cwd=m["cwd"], env={**os.environ, "HOME": a.slot_home(slot), "AGYLOOP_SLOT": slot},
                                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                text=True, bufsize=1)
        s.proc = proc
        err = []
        reader = threading.Thread(target=lambda: err.append(proc.stderr.read()), daemon=True)
        reader.start()
        texts, before, tail, result, denied = {}, {}, [], {}, []
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
                self._step(s, turn, d.get("step_update") or {}, texts, before, denied)
            else:
                tail.append(line)
                if ev == "result":
                    result = d.get("result") or {}
        proc.wait()
        reader.join(timeout=5)
        s.proc = None
        err = "".join(err).strip()
        stopped = s.stop.is_set()
        quota = not stopped and a.is_quota_error(proc.returncode, "".join(tail), err)
        ok = not stopped and not quota and proc.returncode == 0 and result.get("status") == "SUCCESS"
        conv_lost = (resumed and not ok and not quota and not stopped
                     and bool(re.search(r"conversation", err + json.dumps(result), re.I)))
        usage = result.get("usage") or {}
        m["usage"]["input"] += usage.get("input_tokens", 0)
        m["usage"]["output"] += usage.get("output_tokens", 0)
        if result.get("denied_actions") or denied:
            self.notice(s, "denied", "agy denied a tool call that headless mode cannot ask about.",
                        actions=result.get("denied_actions") or [], commands=denied)
        elif err and not ok and not quota and not stopped and not conv_lost:
            self.notice(s, "error", err[-4000:])
        status = ("stopped" if stopped else "quota" if quota else
                  result.get("status") or f"exit {proc.returncode}")
        self.publish(sid, {"t": "turn_end", "turn": turn, "slot": slot, "status": status, "usage": usage,
                           "secs": round(time.time() - t0, 1)}, persist=True)
        self.save(s)
        final = result.get("response") or (texts[max(texts)] if texts else "")
        return {"ok": ok, "quota": quota, "conv_lost": conv_lost, "text": final}

    def _step(self, s, turn, su, texts, before, denied):
        sid, step, state = s.meta["id"], su.get("step_index"), su.get("state")
        kind = su.get("step_type")
        if kind == "agent_response":
            delta = su.get("text_delta") or ""
            if delta:
                texts[step] = texts.get(step, "") + delta
                self.publish(sid, {"t": "delta", "turn": turn, "step": step, "text": delta})
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
                      output=str(info.get("output") or "")[:OUT_CAP])
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
                ev["diff"] = _diff(path, b, after, s.meta["cwd"])
                self._remember(s, path, after)
            self.publish(sid, ev, persist=True)

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
        model = g.get("verifier_model") or m["model"]
        bucket = a.bucket_of(["--model", model])
        self.status(s, "verifying")
        slot, fd = a.acquire(bucket, stop=s.stop)
        if slot is None:
            self.notice(s, "verdict", f"No account has {a.BUCKETS[bucket]} quota for the verifier.", done=None)
            return None
        # Headless agy ends the turn on any denied tool, so the verifier sticks to viewing files.
        prompt = ("You are a strict, independent verifier. Another agent claims this objective is complete:\n\n"
                  f"{g['objective']}\n\nThe work is in the current directory. Inspect it yourself with file "
                  "viewing and listing tools only: shell commands and edits are blocked here and would end your "
                  "turn. Do not modify anything, do not plan fixes, and do not ask questions: only judge. List "
                  "anything missing or broken as '- ' bullets, then end with exactly one final line: "
                  "VERDICT: DONE if the objective is fully achieved, or VERDICT: NOT_DONE otherwise.")
        try:
            a.sync(a.slot_home(slot))
            proc = subprocess.Popen([a.agy_bin(), "-p", prompt, "--output-format", "json", "--model", model],
                                    cwd=m["cwd"], env={**os.environ, "HOME": a.slot_home(slot), "AGYLOOP_SLOT": slot},
                                    stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            s.proc = proc
            out, err = proc.communicate()
            s.proc = None
            if a.is_quota_error(proc.returncode, out, err):
                a.wall_after_quota_error(slot, bucket)
                self.notice(s, "verdict", f"The verifier hit the {a.BUCKETS[bucket]} quota on {slot}.", done=None)
                return None
            reply = json.loads(out)["response"]
        except (ValueError, KeyError, TypeError):
            reply = ""
        finally:
            os.close(fd)
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
