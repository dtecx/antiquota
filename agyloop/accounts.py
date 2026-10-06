"""Several Google accounts behind agy CLI, sharing agy-loop's own stack.

A slot is a fake HOME for one account, named by its email. Its only private
files are agy's auth and per-process state. Its .gemini comes from agy-loop's
own stack (<base>/stack/.gemini, seeded once from ~/.gemini), so skills, hooks,
plugins, MCP config and permissions are the same for every account yet separate
from your own agy CLI and the Antigravity IDE. Every other HOME entry (git, ssh,
gitnexus and claude-mem data, Library) links to the real HOME.
"""
import fcntl
import fnmatch
import json
import os
import pty
import pwd
import re
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
from datetime import datetime

AUTH = ".gemini/antigravity-cli/jetski_state.pbtxt"
# Rebuilt per slot as real directories; their entries are linked one by one.
FARM_DIRS = {".gemini", ".gemini/antigravity-cli", ".gemini/antigravity"}
# Per-account or per-process entries inside FARM_DIRS: never linked.
PRIVATE = ("*_state.pbtxt", "installation_id", "log", "cli.log", "crashes", "cache",
           "presence", "updater", "last_check.timestamp")
BUCKETS = {"gemini": "Gemini", "cg": "Claude/GPT"}
WALL_FRACTION = 0.02          # calibration knob: wall a basket at or below 2% left
FALLBACK_WALL = 5 * 3600 + 600
TEMPFAIL = 75                 # EX_TEMPFAIL: out of quota, retry later
QUOTA_RE = re.compile(r"RESOURCE_EXHAUSTED|quota|\b429\b|rate.?limit|usage limit", re.I)


def home():
    # Not $HOME: inside a slot $HOME is the fake one.
    return os.environ.get("AGYLOOP_HOME") or pwd.getpwuid(os.getuid()).pw_dir


def base():
    return os.environ.get("AGYLOOP_DIR") or os.path.join(home(), ".agyloop")


def state(*parts):
    p = os.path.join(base(), "state", *parts)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    return p


def agy_bin():
    # ~/.local/bin first: launchd and cron start with a bare PATH.
    found = os.environ.get("AGYLOOP_AGY") or shutil.which(
        "agy", path=os.path.join(home(), ".local/bin") + os.pathsep + os.environ.get("PATH", ""))
    if not found:
        sys.exit("agyloop: real agy binary not found; set AGYLOOP_AGY")
    return found


def slot_home(name):
    return os.path.join(base(), "slots", name)


def stack_home():
    return os.path.join(base(), "stack")


def slots():
    """Signed-in accounts (emails) in use order."""
    d = os.path.join(base(), "slots")
    names = sorted(n for n in (os.listdir(d) if os.path.isdir(d) else []) if not n.startswith("."))
    return [n for n in names if os.path.isfile(os.path.join(slot_home(n), AUTH))]


# --- agy-loop's own stack ----------------------------------------------------

# Everything in ~/.gemini is cloned except identity, per-process state and history.
CLONE_SKIP = {
    "": {"antigravity"},  # the IDE's own folder: copy items from it on demand (stack.copy)
    "antigravity-cli": {"conversations", "brain", "annotations", "implicit", "history.jsonl",
                        "bin", "builtin", "scratch"},
    "config": {"projects"},
}
HISTORY = ("conversation_summaries.db*", "*_summaries_proto.pb")
# Made up front so every account links the same ones (one conversation store for handoffs).
STACK_SHARED = (".gemini/antigravity-cli/conversations", ".gemini/antigravity-cli/brain",
                ".gemini/antigravity-cli/annotations", ".gemini/antigravity-cli/implicit")
# Files whose absolute ~/.gemini paths must point into the stack after a copy.
PATHFUL = (".gemini/config/hooks.json", ".gemini/config/mcp_config.json", ".gemini/settings.json")


def to_stack_paths(text):
    """Point absolute paths into your agy config at agy-loop's copy instead (the IDE folder stays)."""
    real, own = os.path.join(home(), ".gemini"), os.path.join(stack_home(), ".gemini")
    for sub in ("config/", "antigravity-cli/", "GEMINI.md", "settings.json"):
        text = text.replace(f"{real}/{sub}", f"{own}/{sub}")
    return text


def _clone_ignore(root):
    def ignore(d, names):
        rel = os.path.relpath(d, root)
        rel = "" if rel == "." else rel
        skip = set(CLONE_SKIP.get(rel, ()))
        if rel in ("", "antigravity-cli"):
            skip |= {n for n in names if any(fnmatch.fnmatch(n, p) for p in PRIVATE + HISTORY)}
        return skip & set(names)
    return ignore


def _replace(src, dst, ignore=None):
    if os.path.isdir(dst) and not os.path.islink(dst):
        shutil.rmtree(dst)
    elif os.path.lexists(dst):
        os.remove(dst)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    if os.path.isdir(src):
        # Dereference links (gitnexus skills live in the IDE folder): the stack owns its copy.
        shutil.copytree(src, dst, ignore=ignore, ignore_dangling_symlinks=True)
    else:
        shutil.copy2(src, dst)


def clone_stack():
    """Copy your whole agy setup into agy-loop's stack. Re-running replaces the config and keeps
    agy-loop's own conversations; your ~/.gemini is only read."""
    src, dst = os.path.join(home(), ".gemini"), os.path.join(stack_home(), ".gemini")
    os.makedirs(dst, exist_ok=True)
    ignore = _clone_ignore(src)
    if os.path.isdir(src):
        for name in set(os.listdir(src)) - ignore(src, os.listdir(src)):
            if name == "antigravity-cli":  # merge one level down so agy-loop's history survives
                cli = os.path.join(src, name)
                for n2 in set(os.listdir(cli)) - ignore(cli, os.listdir(cli)):
                    _replace(os.path.join(cli, n2), os.path.join(dst, name, n2), ignore)
            else:
                _replace(os.path.join(src, name), os.path.join(dst, name), ignore)
    for rel in STACK_SHARED:
        os.makedirs(os.path.join(stack_home(), rel), exist_ok=True)
    for rel in PATHFUL:
        p = os.path.join(stack_home(), rel)
        if os.path.isfile(p):
            with open(p) as f:
                text = f.read()
            with open(p, "w") as f:
                f.write(to_stack_paths(text))


def seed_stack(force=False):
    if force or not os.path.isdir(os.path.join(stack_home(), ".gemini")):
        clone_stack()


def _src(rel):
    """Where a slot entry comes from: the stack for .gemini, the real HOME for the rest."""
    return os.path.join(stack_home() if rel.split("/")[0] == ".gemini" else home(), rel)


def _entries(rel):
    d = _src(rel) if rel else home()
    names = [n for n in os.listdir(d) if not (rel == "" and n == ".gemini")]
    if rel == "" and os.path.isdir(_src(".gemini")):
        names.append(".gemini")
    return names


def _private(rel, name):
    return (rel in FARM_DIRS and any(fnmatch.fnmatch(name, p) for p in PRIVATE)) or (
        os.path.realpath(_src(os.path.join(rel, name))) == os.path.realpath(base()))


def sync(slot_dir, rel=""):
    """Link the stack (.gemini) and the real HOME (the rest) into a slot, minus per-account state."""
    seed_stack()
    here = os.path.join(slot_dir, rel)
    if os.path.islink(here):          # never write links through into the stack or the real HOME
        os.unlink(here)
    os.makedirs(here, exist_ok=True)
    for name in _entries(rel):
        sub = os.path.join(rel, name)
        src, f = _src(sub), os.path.join(slot_dir, sub)
        if sub in FARM_DIRS and os.path.isdir(src):
            sync(slot_dir, sub)
        elif not _private(rel, name) and not os.path.lexists(f):
            os.symlink(src, f)
    for name in os.listdir(here):     # drop links whose target is gone
        f = os.path.join(here, name)
        if os.path.islink(f) and not os.path.exists(f):
            os.unlink(f)


def unshared(slot_dir, rel=""):
    """Entries the slot does not resolve to the stack or the real HOME."""
    bad = []
    for name in _entries(rel):
        sub = os.path.join(rel, name)
        if sub in FARM_DIRS and os.path.isdir(_src(sub)):
            bad += unshared(slot_dir, sub)
        elif not _private(rel, name) and os.path.realpath(os.path.join(slot_dir, sub)) != os.path.realpath(_src(sub)):
            bad.append(sub)
    return bad


def probe_home():
    """An unsigned slot for asking agy about the stack itself (/skills, mcp list, ...)."""
    return os.path.join(base(), "slots", ".stack-probe")


def loaded(slot=None):
    """Skill and hook names agy itself reports for a slot, or for the bare stack (no tokens, no sign-in)."""
    h = slot_home(slot) if slot else probe_home()
    sync(h)
    out = {}
    for cmd, key in (("/skills", "skills"), ("/hooks", "hooks")):
        try:
            r = subprocess.run([agy_bin(), "-p", cmd, "--output-format", "json"], env={**os.environ, "HOME": h},
                               capture_output=True, text=True, timeout=90, stdin=subprocess.DEVNULL)
            out[key] = sorted(x["name"] for x in json.loads(r.stdout)["command"]["data"][key])
        except (subprocess.TimeoutExpired, ValueError, KeyError, TypeError):
            out[key] = None
    return out


SIGNIN_URL = re.compile(r"https://accounts\.google\.com/\S+")


SIGNED_IN_AS = re.compile(r"authenticated successfully as (\S+@\S+)")


def check_name(name):
    if not re.fullmatch(r"[A-Za-z0-9._%+@-]{1,100}", name or "") or name.startswith("."):
        raise ValueError(f"not an account: {name!r}")


class Login:
    """One Google sign-in for a slot, the way agy does it headless.

    Unsigned, `agy -p /quota` prints a Google sign-in URL on stderr and waits for
    the authorization code that Google shows after sign-in. It only prompts when
    stdin is a terminal, so it gets a pseudo-terminal. It runs under an empty
    HOME, so the sign-in cannot write through links into the shared stack.
    The slot is created only after agy proves the sign-in by reading the quota,
    and is named by the email agy logs ("authenticated successfully as ...").
    """

    def __init__(self):
        self.name, self.url, self.error = None, None, None
        slots_dir = os.path.join(base(), "slots")
        os.makedirs(slots_dir, mode=0o700, exist_ok=True)
        self.tmp = tempfile.mkdtemp(prefix=".login-", dir=slots_dir)
        self.log = os.path.join(self.tmp, "signin.log")
        self.pty, tty = pty.openpty()
        self.proc = subprocess.Popen([agy_bin(), "-p", "/quota", "--output-format", "json", "--log-file", self.log],
                                     env={**os.environ, "HOME": self.tmp}, cwd=self.tmp, stdin=tty,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                                     start_new_session=True)
        os.close(tty)
        self.out, self.err = [], []
        self.seen_url = threading.Event()
        self.ended = threading.Event()
        threading.Thread(target=self._pump, args=(self.proc.stdout, self.out), daemon=True).start()
        threading.Thread(target=self._pump, args=(self.proc.stderr, self.err), daemon=True).start()
        threading.Thread(target=self._finish, daemon=True).start()

    def _pump(self, src, sink):
        for line in src:
            sink.append(line)
            m = SIGNIN_URL.search(line)
            if m and not self.url:
                self.url = m.group(0)
                self.seen_url.set()

    def wait_url(self, timeout=30):
        """The sign-in URL, or None if agy ended first (self.error then says why)."""
        for _ in range(int(timeout * 10)):
            if self.seen_url.wait(0.1) or self.ended.is_set():
                break
        return self.url

    def submit(self, code, timeout=90):
        code = (code or "").strip()
        if not code or len(code) > 2000 or any(c in code for c in "\r\n\x00"):
            raise ValueError("paste the single-line code Google showed after sign-in")
        if self.ended.is_set():
            raise ValueError(self.error or "this sign-in already finished")
        os.write(self.pty, (code + "\n").encode())
        self.ended.wait(timeout)
        return self.state

    @property
    def state(self):
        return "waiting" if not self.ended.is_set() else ("failed" if self.error else "done")

    def cancel(self):
        if self.proc.poll() is None:
            self.proc.kill()
        self.ended.wait(5)

    def _finish(self):
        self.proc.wait()
        time.sleep(0.2)  # let the pumps drain
        try:
            os.close(self.pty)
        except OSError:
            pass
        try:
            q = parse_quota(json.loads("".join(self.out)))
        except (ValueError, KeyError, TypeError):
            lines = [ln.strip() for ln in self.err if ln.strip() and not SIGNIN_URL.search(ln)]
            self.error = (lines[-1] if lines else "") or "sign-in failed or timed out"
            shutil.rmtree(self.tmp, ignore_errors=True)
            self.ended.set()
            return
        try:
            with open(self.log, errors="replace") as f:
                found = SIGNED_IN_AS.findall(f.read())
        except OSError:
            found = []
        self.name = found[-1].rstrip(".,;") if found else f"account-{int(time.time())}"
        dst = slot_home(self.name)
        os.makedirs(os.path.dirname(os.path.join(dst, AUTH)), exist_ok=True)
        os.replace(os.path.join(self.tmp, AUTH), os.path.join(dst, AUTH))
        # Keep what else sign-in created: it shows which files are per-account.
        leftover = os.path.join(dst, ".agyloop-login")
        shutil.rmtree(leftover, ignore_errors=True)
        os.replace(self.tmp, leftover)
        sync(dst)
        for b in BUCKETS:
            clear_wall(self.name, b)
        with open(state("quota", self.name + ".json"), "w") as f:
            json.dump({"at": time.time(), "quota": q}, f)
        self.ended.set()


def login():
    """Terminal sign-in: open the URL, paste the code (or let agy notice the sign-in itself)."""
    lg = Login()
    url = lg.wait_url()
    if not url:
        sys.exit(f"agyloop: {lg.error or 'agy printed no sign-in URL'}")
    print(f"Sign in with the Google account to add:\n\n  {url}\n")
    try:
        import webbrowser
        webbrowser.open(url)
    except Exception:
        pass
    print("Paste the code Google shows after sign-in, then press Enter (agy allows 60 seconds):")
    while not lg.ended.is_set():
        if select.select([sys.stdin], [], [], 0.5)[0]:
            line = sys.stdin.readline()
            if not line:  # stdin closed: only agy noticing the sign-in itself can finish now
                lg.ended.wait()
                break
            try:
                lg.submit(line)
            except ValueError as e:
                print(e)
    if lg.error:
        sys.exit(f"agyloop: {lg.error}")
    print(f"{lg.name} signed in; it uses agy-loop's stack in {stack_home()}")


def remove(name):
    check_name(name)
    p = slot_home(name)
    if not os.path.isdir(p) or os.path.dirname(p) != os.path.join(base(), "slots"):
        raise KeyError(name)
    fd = try_lease(name)
    if fd is None:
        raise ValueError(f"'{name}' is running a call; stop it first")
    try:
        shutil.rmtree(p)
        for b in BUCKETS:
            clear_wall(name, b)
    finally:
        os.close(fd)


# --- walls and leases ---------------------------------------------------------

def walled_until(slot, bucket):
    try:
        with open(state("walls", f"{slot}.{bucket}")) as f:
            t = float(f.read().split()[0])
    except (OSError, ValueError, IndexError):
        return 0
    return t if t > time.time() else 0


def set_wall(slot, bucket, until, why):
    p = state("walls", f"{slot}.{bucket}")
    with open(p + ".tmp", "w") as f:
        f.write(f"{int(until)} {why}\n")
    os.replace(p + ".tmp", p)


def clear_wall(slot, bucket):
    try:
        os.remove(state("walls", f"{slot}.{bucket}"))
    except FileNotFoundError:
        pass


def try_lease(slot):
    """Exclusive per-slot lock; concurrent agy on one HOME races the token refresh."""
    fd = os.open(state("leases", slot + ".lock"), os.O_RDWR | os.O_CREAT, 0o600)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return fd
    except OSError:
        os.close(fd)
        return None


def acquire(bucket, pin=None, stop=None):
    """Lease the first live slot. Sticky: one account drains before the next starts.

    (None, None) when every slot is walled, or when `stop` (an Event) is set."""
    while not (stop and stop.is_set()):
        live = [s for s in ([pin] if pin else slots()) if not walled_until(s, bucket)]
        if not live:
            return None, None
        for s in live:
            fd = try_lease(s)
            if fd is not None:
                return s, fd
        time.sleep(1)  # ponytail: polling, fine for a handful of accounts
    return None, None


def next_reset(bucket):
    """Earliest time a walled slot comes back for this bucket (0 if none is walled)."""
    return min((w for w in (walled_until(s, bucket) for s in slots()) if w), default=0)


def wall_after_quota_error(slot, bucket):
    # The quota panel knows the real reset; fall back to the 5-hour window.
    if probe_slot(slot) is None or not walled_until(slot, bucket):
        set_wall(slot, bucket, time.time() + FALLBACK_WALL, "quota error")


def bucket_of(args):
    model = next((a.split("=", 1)[1] for a in args if a.startswith("--model=")), "")
    if "--model" in args and args.index("--model") + 1 < len(args):
        model = args[args.index("--model") + 1]
    # ponytail: no --model means the agy default, assumed Gemini; the loop always passes --model.
    return "cg" if model.startswith(("claude", "gpt")) else "gemini"


# --- quota --------------------------------------------------------------------

def parse_quota(d):
    """{bucket: {window: (remaining_fraction, reset_epoch)}} from `agy -p /quota --output-format json`."""
    out = {}
    for g in d["command"]["data"]["groups"]:
        bucket = "gemini" if g["name"].startswith("Gemini") else "cg"
        out[bucket] = {b["window"]: (float(b["remaining_fraction"]),
                                     datetime.fromisoformat(b["reset_time"]).timestamp())
                       for b in g["buckets"]}
    return out


def probe_slot(slot):
    """Read /quota (no model tokens), set or clear walls from it. None if unreadable."""
    try:
        r = subprocess.run([agy_bin(), "-p", "/quota", "--output-format", "json"],
                           env={**os.environ, "HOME": slot_home(slot)},
                           capture_output=True, text=True, timeout=90)
        q = parse_quota(json.loads(r.stdout))
    except (subprocess.TimeoutExpired, ValueError, KeyError, TypeError):
        return None
    with open(state("quota", slot + ".json"), "w") as f:
        json.dump({"at": time.time(), "quota": q}, f)
    for bucket, windows in q.items():
        spent = [reset for frac, reset in windows.values() if frac <= WALL_FRACTION]
        if spent:
            set_wall(slot, bucket, max(spent) + 120, "quota panel")
        else:
            clear_wall(slot, bucket)
    return q


def probe():
    for s in slots():
        fd = try_lease(s)
        if fd is None:
            print(f"{s}: busy, skipped")
            continue
        try:
            sync(slot_home(s))
            print(f"{s}: " + ("unreadable" if probe_slot(s) is None else "ok"))
        finally:
            os.close(fd)


# --- run ----------------------------------------------------------------------

def _tee(cmd, env):
    """Run cmd, pass its output through, keep the last 64 KB of each stream."""
    p = subprocess.Popen(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    tails = {}

    def pump(src, dst, key):
        buf = b""
        for chunk in iter(lambda: src.read1(65536), b""):
            dst.write(chunk)
            dst.flush()
            buf = (buf + chunk)[-65536:]
        tails[key] = buf

    threads = [threading.Thread(target=pump, args=(p.stdout, sys.stdout.buffer, "out")),
               threading.Thread(target=pump, args=(p.stderr, sys.stderr.buffer, "err"))]
    for t in threads:
        t.start()
    p.wait()
    for t in threads:
        t.join()
    return p.returncode, tails["out"].decode(errors="replace"), tails["err"].decode(errors="replace")


def is_quota_error(rc, out, err):
    """Only agy's own error channels count, never the model's answer text."""
    if rc != 0 and QUOTA_RE.search(err):
        return True
    for chunk in [out, *out.splitlines()]:
        try:
            d = json.loads(chunk)
        except ValueError:
            continue
        if isinstance(d, dict) and isinstance(d.get("result"), dict):  # stream-json result event
            d = d["result"]
        if isinstance(d, dict) and (d.get("type") == "error" or d.get("error")
                                    or d.get("status") not in (None, "SUCCESS")):
            if QUOTA_RE.search(json.dumps({k: v for k, v in d.items() if k != "response"})):
                return True
    return False


def run(args, pin=None):
    if not slots():
        sys.exit("agyloop: no signed-in account; run: agyloop login")
    if pin and pin not in slots():
        sys.exit(f"agyloop: no signed-in slot '{pin}'")
    agy, bucket = agy_bin(), bucket_of(args)
    slot, fd = acquire(bucket, pin)
    if slot is None:
        nxt = min(walled_until(s, bucket) for s in ([pin] if pin else slots()))
        print(f"agyloop: every account is out of {BUCKETS[bucket]} quota; next reset "
              f"{datetime.fromtimestamp(nxt):%m-%d %H:%M}", file=sys.stderr)
        return TEMPFAIL
    try:
        sync(slot_home(slot))
        env = {**os.environ, "HOME": slot_home(slot), "AGYLOOP_SLOT": slot}
        t0 = time.time()
        if sys.stdout.isatty():  # interactive: agy needs the real terminal, no capture
            old = signal.signal(signal.SIGINT, signal.SIG_IGN)
            try:
                rc = subprocess.run([agy, *args], env=env).returncode
            finally:
                signal.signal(signal.SIGINT, old)
            out = err = ""
        else:
            rc, out, err = _tee([agy, *args], env)
        walled = is_quota_error(rc, out, err)
        if walled:
            wall_after_quota_error(slot, bucket)
        if rc != 0:  # keep raw failures: the real error shapes are still being learned
            with open(state("failures", f"{int(t0)}-{slot}.log"), "w") as f:
                f.write(f"rc={rc}\n--- stdout\n{out}\n--- stderr\n{err}\n")
        with open(state("calls.jsonl"), "a") as f:
            f.write(json.dumps({"t": int(t0), "slot": slot, "bucket": bucket, "rc": rc,
                                "secs": round(time.time() - t0, 1), "walled": walled}) + "\n")
        return TEMPFAIL if walled else rc
    finally:
        os.close(fd)
