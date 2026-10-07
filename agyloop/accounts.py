"""Several Google accounts behind agy CLI, sharing agy-loop's own stack.

A slot is a fake HOME for one account, named by its email. agy normally keeps
its sign-in token in the OS keyring, which belongs to the OS user, not to HOME.
But when it sees an SSH session it stores the token in a file instead
("Using file-based token storage because SSH session detected"): in
.gemini/antigravity-cli/antigravity-oauth-token under its HOME. agy-loop runs every agy
call that way, so each account's token is a plain file in its own slot, on
macOS, Linux and Windows alike, headless or not, and your own agy CLI's keyring
entry is never read or written. A slot is portable: copy the folder to another
machine and the account comes with it.

A slot's .gemini comes from agy-loop's own stack (<base>/stack/.gemini, cloned
from ~/.gemini), so skills, hooks, plugins, MCP config and permissions are the
same for every account yet separate from your agy CLI and the Antigravity IDE.
Every other HOME entry (git, ssh, gitnexus and claude-mem data) links to the real HOME.
"""
import fnmatch
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
from datetime import datetime

try:  # POSIX
    import fcntl
    import pty
    import pwd
except ImportError:  # Windows
    fcntl = pty = pwd = None
    import msvcrt

# agy's file token storage (SSH mode); jetski_state.pbtxt next to it is per-account state too.
AUTH = ".gemini/antigravity-cli/antigravity-oauth-token"
ACCOUNT_FILES = (AUTH, ".gemini/antigravity-cli/jetski_state.pbtxt")
# Rebuilt per slot as real directories; their entries are linked one by one.
FARM_DIRS = {".gemini", ".gemini/antigravity-cli", ".gemini/antigravity"}
# Per-account or per-process entries inside FARM_DIRS: never linked.
PRIVATE = ("antigravity-oauth-token*", "*_state.pbtxt", "installation_id", "log", "cli.log", "crashes", "cache",
           "presence", "updater", "last_check.timestamp")
# agy picks file token storage when it sees an SSH session; the value itself is not used.
FILE_TOKENS = {"SSH_CONNECTION": "127.0.0.1 0 127.0.0.1 0"}
BUCKETS = {"gemini": "Gemini", "cg": "Claude/GPT"}
WALL_FRACTION = 0.02          # calibration knob: wall a basket at or below 2% left
FALLBACK_WALL = 5 * 3600 + 600
TEMPFAIL = 75                 # EX_TEMPFAIL: out of quota, retry later
QUOTA_RE = re.compile(r"RESOURCE_EXHAUSTED|quota|\b429\b|rate.?limit|usage limit", re.I)


def home():
    # Not $HOME on POSIX: inside a slot $HOME is the fake one.
    if os.environ.get("AGYLOOP_HOME"):
        return os.environ["AGYLOOP_HOME"]
    return pwd.getpwuid(os.getuid()).pw_dir if pwd else os.path.expanduser("~")


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


def agy_cmd(home_dir, args, env=None):
    """(cmd, env) to run agy as the account in home_dir, its token kept in that folder."""
    env = {**(env or os.environ), **FILE_TOKENS, "HOME": home_dir, "USERPROFILE": home_dir}
    return [agy_bin(), *args], env


def link(src, dst):
    """Symlink; on Windows without symlink rights a junction (folders) or hard link (files)."""
    try:
        os.symlink(src, dst, target_is_directory=os.path.isdir(src))
    except OSError:
        if os.name != "nt":
            raise
        if os.path.isdir(src):
            import _winapi
            _winapi.CreateJunction(src, dst)
        else:
            try:
                os.link(src, dst)
            except OSError:
                shutil.copy2(src, dst)


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
    expose_usage()


def usage_link():
    return os.path.join(home(), ".config", "antigravity")


def expose_usage():
    """Let usage tools see agy-loop's conversations: ccusage (and so `hacklab sync`) reads
    Antigravity token usage from a few fixed folders, ~/.config/antigravity among them, and
    nothing uses that one. Point it at agy-loop's conversation store. True if it is in place."""
    link_path, target = usage_link(), os.path.join(stack_home(), ".gemini", "antigravity-cli")
    if os.environ.get("AGYLOOP_NO_USAGE_LINK"):
        return False
    if os.path.islink(link_path):
        if os.path.realpath(link_path) == os.path.realpath(target):
            return True
        if not os.path.exists(link_path):  # a dangling link of ours from an older location
            os.unlink(link_path)
        else:
            return False
    elif os.path.exists(link_path):
        return False  # something else owns it: leave it alone
    os.makedirs(os.path.dirname(link_path), exist_ok=True)
    link(target, link_path)
    return True


def _join(rel, name):
    return f"{rel}/{name}" if rel else name


def _src(rel):
    """Where a slot entry comes from: the stack for .gemini, the real HOME for the rest."""
    return os.path.join(stack_home() if rel.split("/")[0] == ".gemini" else home(), *rel.split("/"))


def _entries(rel):
    d = _src(rel) if rel else home()
    names = [n for n in os.listdir(d) if not (rel == "" and n == ".gemini")]
    if rel == "" and os.path.isdir(_src(".gemini")):
        names.append(".gemini")
    return names


def _private(rel, name):
    return (rel in FARM_DIRS and any(fnmatch.fnmatch(name, p) for p in PRIVATE)) or (
        os.path.realpath(_src(_join(rel, name))) == os.path.realpath(base()))


def sync(slot_dir, rel=""):
    """Link the stack (.gemini) and the real HOME (the rest) into a slot, minus per-account state."""
    seed_stack()
    here = os.path.join(slot_dir, *rel.split("/")) if rel else slot_dir
    if os.path.islink(here):          # never write links through into the stack or the real HOME
        os.unlink(here)
    os.makedirs(here, exist_ok=True)
    for name in _entries(rel):
        sub = _join(rel, name)
        src, f = _src(sub), os.path.join(here, name)
        if sub in FARM_DIRS and os.path.isdir(src):
            sync(slot_dir, sub)
        elif not _private(rel, name) and not os.path.lexists(f):
            link(src, f)
    for name in os.listdir(here):     # drop links whose target is gone
        f = os.path.join(here, name)
        if os.path.islink(f) and not os.path.exists(f):
            os.unlink(f)


def unshared(slot_dir, rel=""):
    """Entries the slot does not resolve to the stack or the real HOME."""
    bad = []
    for name in _entries(rel):
        sub = _join(rel, name)
        if sub in FARM_DIRS and os.path.isdir(_src(sub)):
            bad += unshared(slot_dir, sub)
        elif not _private(rel, name) and os.path.realpath(os.path.join(slot_dir, *sub.split("/"))) != os.path.realpath(_src(sub)):
            bad.append(sub)
    return bad


def probe_home():
    """Where to ask agy about the stack itself (/skills, mcp list, ...): a signed-in account if
    there is one, since in file-token mode an unsigned agy stops to wait for a sign-in."""
    if slots():
        return slot_home(slots()[0])
    return os.path.join(base(), "slots", ".stack-probe")


def loaded(slot=None):
    """Skill and hook names agy itself reports for a slot, or for the bare stack (no tokens, no sign-in)."""
    h = slot_home(slot) if slot else probe_home()
    if not slot and not slots():  # nobody to ask: read the stack's own folders instead
        cfg = os.path.join(stack_home(), ".gemini/config")
        try:
            with open(os.path.join(cfg, "hooks.json")) as f:
                hooks = sorted(json.load(f))
        except (OSError, ValueError):
            hooks = []
        skills = os.path.join(cfg, "skills")
        return {"skills": sorted(os.listdir(skills)) if os.path.isdir(skills) else [], "hooks": hooks}
    sync(h)
    out = {}
    for cmd, key in (("/skills", "skills"), ("/hooks", "hooks")):
        try:
            c, env = agy_cmd(h, ["-p", cmd, "--output-format", "json"])
            r = subprocess.run(c, env=env, capture_output=True, text=True, timeout=90, stdin=subprocess.DEVNULL)
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
        self.name, self.url, self.error, self.cancelled, self.verify_url = None, None, None, None, None
        slots_dir = os.path.join(base(), "slots")
        os.makedirs(slots_dir, mode=0o700, exist_ok=True)
        self.tmp = tempfile.mkdtemp(prefix=".login-", dir=slots_dir)
        self.log = os.path.join(self.tmp, "signin.log")
        # agy reads the pasted code only from a terminal. Without a pty module (Windows) it
        # still finishes by itself once the browser sign-in completes.
        self.pty, tty = pty.openpty() if pty else (None, subprocess.DEVNULL)
        cmd, env = agy_cmd(self.tmp, ["-p", "/quota", "--output-format", "json", "--log-file", self.log])
        self.proc = subprocess.Popen(cmd, env=env, cwd=self.tmp, stdin=tty,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                                     start_new_session=os.name != "nt")
        if self.pty is not None:
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
        if self.pty is None:
            raise ValueError("on this system agy finishes sign-in by itself; just complete it in the browser")
        os.write(self.pty, (code + "\n").encode())
        self.ended.wait(timeout)
        return self.state

    def cancel(self, why="cancelled"):
        self.cancelled = why
        if self.proc.poll() is None:
            self.proc.kill()
        self.ended.wait(5)

    @property
    def state(self):
        if not self.ended.is_set():
            return "waiting"
        return "verify" if self.verify_url else "failed" if self.error else "done"

    def _finish(self):
        self.proc.wait()
        time.sleep(0.2)  # let the pumps drain
        try:
            if self.pty is not None:
                os.close(self.pty)
        except OSError:
            pass
        out = "".join(self.out)
        try:  # agy may print more than the JSON after a fresh sign-in
            d = json.loads(out[out.index("{"): out.rindex("}") + 1])
        except ValueError:
            d = {}
        try:
            with open(self.log, errors="replace") as f:
                found = SIGNED_IN_AS.findall(f.read())
        except OSError:
            found = []
        email = found[-1].rstrip(".,;") if found else None
        if d.get("status") == "SUCCESS":
            self._adopt(email or f"account-{int(time.time())}", d)
        elif email and os.path.exists(os.path.join(self.tmp, AUTH)) and VERIFY_URL.search(str(d.get("error"))):
            # Signed in, but Google wants this account verified before Antigravity serves it.
            # The token is good: keep the account, parked until the verification is done.
            self.verify_url = VERIFY_URL.search(d["error"]).group(0)
            self.error = d["error"].split("\n")[0]
            self._adopt(email, None)
            mark_verify(email, self.verify_url)
        else:
            # agy's own error line, never one of its prompts; a killed agy prints none.
            errors = [ln.strip() for ln in self.err if re.match(r"\s*error", ln, re.I)]
            self.error = self.cancelled or (errors[-1] if errors else "the sign-in ended before it finished")
            try:  # keep the evidence: sign-in failures are hard to reproduce
                with open(state("failures", f"signin-{int(time.time())}.log"), "w") as f:
                    f.write(f"error={self.error}\nrc={self.proc.returncode}\n--- stdout\n{out[-4000:]}\n--- stderr\n"
                            + "".join(ln for ln in self.err if not SIGNIN_URL.search(ln)))
            except OSError:
                pass
            shutil.rmtree(self.tmp, ignore_errors=True)
        self.ended.set()

    def _adopt(self, name, result):
        """Move the signed-in account from the temporary HOME into its slot."""
        self.name = name
        dst = slot_home(name)
        os.makedirs(os.path.dirname(os.path.join(dst, AUTH)), exist_ok=True)
        for rel in ACCOUNT_FILES:
            if os.path.exists(os.path.join(self.tmp, rel)):
                os.replace(os.path.join(self.tmp, rel), os.path.join(dst, rel))
        # Keep what else sign-in created: it shows which files are per-account.
        leftover = os.path.join(dst, ".agyloop-login")
        shutil.rmtree(leftover, ignore_errors=True)
        os.replace(self.tmp, leftover)
        sync(dst)
        for b in BUCKETS:
            clear_wall(name, b)
        clear_verify(name)
        try:  # a quota snapshot is a bonus; the sign-in already succeeded
            q = parse_quota(result)
            with open(state("quota", name + ".json"), "w") as f:
                json.dump({"at": time.time(), "quota": q}, f)
        except (ValueError, KeyError, TypeError, AttributeError):
            pass


# --- accounts Google wants verified first -----------------------------------------

VERIFY_URL = re.compile(r"https://accounts\.google\.com/signin/continue\S+")


def mark_verify(slot, url):
    """Park an account that must be verified with Google: walled until a probe succeeds."""
    with open(state("verify", slot + ".json"), "w") as f:
        json.dump({"url": url, "since": time.time()}, f)
    for b in BUCKETS:
        set_wall(slot, b, time.time() + 30 * 86400, "verify the account with Google")


def needs_verify(slot):
    try:
        with open(state("verify", slot + ".json")) as f:
            return json.load(f).get("url")
    except (OSError, ValueError):
        return None


def clear_verify(slot):
    try:
        os.remove(state("verify", slot + ".json"))
    except FileNotFoundError:
        pass


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
    lines = []

    def read():  # a thread, not select(): works on Windows consoles too
        for line in sys.stdin:
            lines.append(line)
    threading.Thread(target=read, daemon=True).start()
    while not lg.ended.wait(0.5):
        while lines:
            try:
                lg.submit(lines.pop(0))
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
        if fcntl:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        else:
            msvcrt.locking(fd, msvcrt.LK_NBLCK, 1)
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
    """{bucket: {window: (remaining_fraction, reset_epoch or None)}} from `agy -p /quota --output-format json`.

    A window agy reports as disabled (e.g. the 5-hour one while the weekly one is spent)
    has no fraction and no reset: it counts as empty, and its wall follows the other window."""
    out = {}
    for g in d["command"]["data"]["groups"]:
        bucket = "gemini" if g["name"].startswith("Gemini") else "cg"
        out[bucket] = {b["window"]: (float(b.get("remaining_fraction") or 0),
                                     datetime.fromisoformat(b["reset_time"]).timestamp() if b.get("reset_time") else None)
                       for b in g["buckets"]}
    return out


def probe_slot(slot):
    """Read /quota (no model tokens), set or clear walls from it. None if unreadable."""
    try:
        cmd, env = agy_cmd(slot_home(slot), ["-p", "/quota", "--output-format", "json"])
        r = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=90, stdin=subprocess.DEVNULL)
        url = VERIFY_URL.search(r.stdout + r.stderr)
        if url and "eligib" in (r.stdout + r.stderr).lower():
            mark_verify(slot, url.group(0))
            return None
        q = parse_quota(json.loads(r.stdout))
    except (subprocess.TimeoutExpired, ValueError, KeyError, TypeError):
        return None
    with open(state("quota", slot + ".json"), "w") as f:
        json.dump({"at": time.time(), "quota": q}, f)
    clear_verify(slot)  # agy answered: Google is happy with this account now
    for bucket, windows in q.items():
        spent = [reset for frac, reset in windows.values() if frac <= WALL_FRACTION]
        resets = [r for r in spent if r] or [r for _, r in windows.values() if r]
        if spent:
            set_wall(slot, bucket, (max(resets) if resets else time.time() + FALLBACK_WALL) + 120, "quota panel")
        else:
            clear_wall(slot, bucket)
    return q


def whoami(slot):
    """The email agy actually signs in as for this slot (checks that its credential store is its own)."""
    log = state("whoami", slot + ".log")
    if os.path.exists(log):
        os.remove(log)
    cmd, env = agy_cmd(slot_home(slot), ["-p", "/quota", "--output-format", "json", "--log-file", log])
    try:
        subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=90, stdin=subprocess.DEVNULL)
        with open(log, errors="replace") as f:
            found = SIGNED_IN_AS.findall(f.read())
    except (OSError, subprocess.TimeoutExpired):
        return None
    return found[-1].rstrip(".,;") if found else None


def usage_today():
    """{slot: {"input", "output", "thinking", "turns"}} since local midnight, from the engine's ledger."""
    midnight = datetime.now().replace(hour=0, minute=0, second=0, microsecond=0).timestamp()
    out = {}
    try:
        with open(state("usage.jsonl")) as f:
            for line in f:
                try:
                    u = json.loads(line)
                except ValueError:
                    continue
                if u.get("t", 0) < midnight:
                    continue
                d = out.setdefault(u["slot"], {"input": 0, "output": 0, "thinking": 0, "turns": 0})
                for k in ("input", "output", "thinking"):
                    d[k] += u.get(k, 0)
                d["turns"] += 1
    except OSError:
        pass
    return out


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
    bucket = bucket_of(args)
    slot, fd = acquire(bucket, pin)
    if slot is None:
        nxt = min(walled_until(s, bucket) for s in ([pin] if pin else slots()))
        print(f"agyloop: every account is out of {BUCKETS[bucket]} quota; next reset "
              f"{datetime.fromtimestamp(nxt):%m-%d %H:%M}", file=sys.stderr)
        return TEMPFAIL
    try:
        sync(slot_home(slot))
        cmd, env = agy_cmd(slot_home(slot), args, {**os.environ, "AGYLOOP_SLOT": slot})
        t0 = time.time()
        if sys.stdout.isatty():  # interactive: agy needs the real terminal, no capture
            old = signal.signal(signal.SIGINT, signal.SIG_IGN)
            try:
                rc = subprocess.run(cmd, env=env).returncode
            finally:
                signal.signal(signal.SIGINT, old)
            out = err = ""
        else:
            rc, out, err = _tee(cmd, env)
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
