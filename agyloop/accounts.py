"""Several Google accounts behind one agy CLI, all sharing one stack.

A slot is a fake HOME for one account. Its only private file is agy's auth
state; every other entry is a symlink back to the real HOME. Skills, hooks,
plugins, MCP servers, rules, claude-mem and gitnexus data, git and ssh config
are therefore identical for every account. "main" is the real HOME itself.
"""
import fcntl
import fnmatch
import json
import os
import pwd
import re
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
    return home() if name == "main" else os.path.join(base(), "slots", name)


def slots():
    """Signed-in slots in use order: main first, then by name."""
    d = os.path.join(base(), "slots")
    names = ["main"] + sorted(n for n in (os.listdir(d) if os.path.isdir(d) else []) if not n.startswith("."))
    return [n for n in names if os.path.isfile(os.path.join(slot_home(n), AUTH))]


# --- shared stack -------------------------------------------------------------

def sync(slot_dir, real=None, rel=""):
    """Link every real-HOME entry into the slot, except per-account state."""
    real = real or home()
    if os.path.islink(slot_dir):          # never write links through into the real HOME
        os.unlink(slot_dir)
    os.makedirs(slot_dir, exist_ok=True)
    for name in os.listdir(real):
        r, f, sub = os.path.join(real, name), os.path.join(slot_dir, name), os.path.join(rel, name)
        if sub in FARM_DIRS and os.path.isdir(r):
            sync(f, r, sub)
        elif os.path.realpath(r) == os.path.realpath(base()):
            continue
        elif rel in FARM_DIRS and any(fnmatch.fnmatch(name, p) for p in PRIVATE):
            continue
        elif not os.path.lexists(f):
            os.symlink(r, f)
    for name in os.listdir(slot_dir):     # drop links whose target is gone
        f = os.path.join(slot_dir, name)
        if os.path.islink(f) and not os.path.exists(f):
            os.unlink(f)


def unshared(slot_dir, real=None, rel=""):
    """Real-HOME entries the slot does not resolve to the same place."""
    real, bad = real or home(), []
    for name in os.listdir(real):
        r, f, sub = os.path.join(real, name), os.path.join(slot_dir, name), os.path.join(rel, name)
        if sub in FARM_DIRS and os.path.isdir(r):
            bad += unshared(f, r, sub)
        elif os.path.realpath(r) == os.path.realpath(base()) or (
                rel in FARM_DIRS and any(fnmatch.fnmatch(name, p) for p in PRIVATE)):
            continue
        elif os.path.realpath(f) != os.path.realpath(r):
            bad.append(sub)
    return bad


def loaded(slot):
    """Skill and hook names agy itself reports under this slot (metadata turns, no tokens)."""
    out = {}
    for cmd, key in (("/skills", "skills"), ("/hooks", "hooks")):
        try:
            r = subprocess.run([agy_bin(), "-p", cmd, "--output-format", "json"],
                               env={**os.environ, "HOME": slot_home(slot)},
                               capture_output=True, text=True, timeout=90)
            out[key] = sorted(x["name"] for x in json.loads(r.stdout)["command"]["data"][key])
        except (subprocess.TimeoutExpired, ValueError, KeyError, TypeError):
            out[key] = None
    return out


def login(name):
    if name == "main" or not re.fullmatch(r"[A-Za-z0-9_-]+", name):
        sys.exit("agyloop: slot name must be letters, digits, - or _ ('main' is your real HOME)")
    slots_dir = os.path.join(base(), "slots")
    os.makedirs(slots_dir, mode=0o700, exist_ok=True)
    # Sign in under an empty HOME, so agy cannot write through links into the shared stack.
    tmp = tempfile.mkdtemp(prefix=".login-", dir=slots_dir)
    print(f"Sign in with the Google account for slot '{name}', then exit agy.")
    subprocess.run([agy_bin()], env={**os.environ, "HOME": tmp})
    if not os.path.isfile(os.path.join(tmp, AUTH)):
        shutil.rmtree(tmp)
        sys.exit("agyloop: no auth state written; run login again and finish the sign-in")
    dst = slot_home(name)
    os.makedirs(os.path.dirname(os.path.join(dst, AUTH)), exist_ok=True)
    os.replace(os.path.join(tmp, AUTH), os.path.join(dst, AUTH))
    # Keep what else sign-in created: it shows which files are per-account.
    leftover = os.path.join(dst, ".agyloop-login")
    shutil.rmtree(leftover, ignore_errors=True)
    os.replace(tmp, leftover)
    sync(dst)
    print(f"slot '{name}' signed in; stack linked from {home()}")


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


def acquire(bucket, pin=None):
    """Lease the first live slot. Sticky: one account drains before the next starts."""
    while True:
        live = [s for s in ([pin] if pin else slots()) if not walled_until(s, bucket)]
        if not live:
            return None, None
        for s in live:
            fd = try_lease(s)
            if fd is not None:
                return s, fd
        time.sleep(1)  # ponytail: polling, fine for a handful of accounts


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
            if s != "main":
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
        if isinstance(d, dict) and (d.get("type") == "error" or d.get("error")
                                    or d.get("status") not in (None, "SUCCESS")):
            if QUOTA_RE.search(json.dumps({k: v for k, v in d.items() if k != "response"})):
                return True
    return False


def run(args, pin=None):
    if not slots():
        sys.exit("agyloop: no signed-in account; sign in to agy or run: agyloop login <name>")
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
        if slot != "main":
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
            # The quota panel knows the real reset; fall back to the 5-hour window.
            if probe_slot(slot) is None or not walled_until(slot, bucket):
                set_wall(slot, bucket, time.time() + FALLBACK_WALL, "quota error")
        if rc != 0:  # keep raw failures: the real error shapes are still being learned
            with open(state("failures", f"{int(t0)}-{slot}.log"), "w") as f:
                f.write(f"rc={rc}\n--- stdout\n{out}\n--- stderr\n{err}\n")
        with open(state("calls.jsonl"), "a") as f:
            f.write(json.dumps({"t": int(t0), "slot": slot, "bucket": bucket, "rc": rc,
                                "secs": round(time.time() - t0, 1), "walled": walled}) + "\n")
        return TEMPFAIL if walled else rc
    finally:
        os.close(fd)
