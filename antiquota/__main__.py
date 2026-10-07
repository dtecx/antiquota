import argparse
import json
import os
import queue
import sys
from datetime import datetime

from . import accounts as a

USAGE = """usage: antiquota <command>

  accounts             signed-in accounts (by email), quota and walls (default)
  login                add a Google account: open the sign-in link, paste the code
  probe                read /quota for every account and wall the spent ones
  doctor               check that every account loads antiquota's own stack
  stack [reseed]       print the stack folder; reseed re-copies config from ~/.gemini
  unwall <name|all>    clear walls by hand
  run [--slot S] [--] <agy args...>
                       run agy on the first account with quota left;
                       exit 75 when every account is out of quota
  gui [--port N] [--no-open]
                       local web GUI (sessions, goal mode, accounts, stack)
  goal <objective> [--cwd D] [--check CMD] [--model M] ...
                       headless goal run in the terminal; the session also
                       shows up in the GUI. See: antiquota goal --help"""


def cmd_accounts():
    rows = a.slots()
    if not rows:
        print("no signed-in account; run: antiquota login")
        return 0
    width = max(10, *(len(s) + 2 for s in rows))
    print(f"{'ACCOUNT':{width}} {'GEMINI':30} {'CLAUDE/GPT':30} LEASE")
    for s in rows:
        try:
            with open(a.state("quota", s + ".json")) as f:
                q = json.load(f)["quota"]
        except (OSError, ValueError, KeyError):
            q = {}
        cells = []
        for b in a.BUCKETS:
            w = a.walled_until(s, b)
            if w:
                cells.append(f"walled to {datetime.fromtimestamp(w):%m-%d %H:%M}")
            elif b in q:
                cells.append("  ".join(f"{win} {frac:.0%}" for win, (frac, _) in q[b].items()))
            else:
                cells.append("free, not probed")
        fd = a.try_lease(s)
        if fd is not None:
            os.close(fd)
        print(f"{s:{width}} {cells[0]:30} {cells[1]:30} {'free' if fd is not None else 'busy'}")
    return 0


def cmd_doctor():
    a.seed_stack()
    gem = os.path.join(a.stack_home(), ".gemini")
    try:
        with open(os.path.join(gem, "config/mcp_config.json")) as f:
            mcp = sorted(json.load(f).get("mcpServers", {}))
    except (OSError, ValueError):
        mcp = []
    ref = a.loaded()
    print(f"antiquota stack in {gem} (separate from ~/.gemini), as agy reports it:")
    print(f"  skills ({len(ref['skills'] or [])}): {', '.join(ref['skills'] or ['<unreadable>'])}")
    print(f"  hooks:   {', '.join(ref['hooks'] or ['<unreadable>'])}")
    print(f"  mcp:     {', '.join(mcp)}")
    print("sign-in tokens: one file per account (agy file storage), never your OS keyring")
    print("conversation store link: " + (f"{a.usage_link()} -> antiquota" if a.expose_usage()
          else f"not set: {a.usage_link()} belongs to something else"))
    problems = 0
    real_auth = os.path.join(a.home(), a.AUTH)
    for s in a.slots():
        sh = a.slot_home(s)
        a.sync(sh)
        bad = a.unshared(sh)
        auth = os.path.join(sh, a.AUTH)
        if os.path.islink(auth) or (os.path.exists(real_auth) and os.path.samefile(auth, real_auth)):
            bad.append(a.AUTH + " (must be private)")
        got = a.loaded(s)
        bad += [f"agy {k} differ" for k in ref if got[k] != ref[k]]
        me = a.whoami(s)
        if me != s:
            bad.append(f"signs in as {me or 'nobody'}: its credential store is not its own")
        print(f"{s}: " + ("ok: signs in as itself, loads the stack's skills and hooks" if not bad
                          else "PROBLEM: " + ", ".join(bad)))
        problems += len(bad)
    return 1 if problems else 0


def cmd_stack(rest):
    if rest == ["reseed"]:
        a.seed_stack(force=True)
        print("config re-copied from ~/.gemini (conversations kept)")
    else:
        a.seed_stack()
    print(os.path.join(a.stack_home(), ".gemini"))
    return 0


def cmd_unwall(target):
    for s in (a.slots() if target == "all" else [target]):
        for b in a.BUCKETS:
            a.clear_wall(s, b)
    print(f"walls cleared: {target}")
    return 0


def cmd_gui(rest, open_browser=True):
    p = argparse.ArgumentParser(prog="antiquota gui" if open_browser else "antiquota serve")
    p.add_argument("--port", type=int, default=8765, help="0 picks a free port")
    p.add_argument("--no-open", action="store_true", help="do not open a browser")
    o = p.parse_args(rest)
    from .server import serve
    return serve(o.port, open_browser and not o.no_open)


def _print_event(ev, cwd):
    t = ev["t"]
    if t == "delta":
        sys.stdout.write(ev["text"])
    elif t == "assistant":
        sys.stdout.write("\n")
    elif t == "user" and ev.get("kind") != "user":
        print(f"\n>> {ev['kind']}: {ev['text'][:400]}")
    elif t == "turn_start":
        print(f"\n-- turn {ev['turn']} on {ev['slot']} ({ev['model']})")
    elif t == "tool" and ev["state"] != "running":
        p = ev.get("params") or {}
        arg = p.get("CommandLine") or p.get("TargetFile") or p.get("AbsolutePath") or p.get("Query") or ""
        print(f"   {'x' if ev['state'] == 'error' else '.'} {ev['name']} {arg.replace(cwd + '/', '')}"[:200])
    elif t == "turn_end":
        print(f"   {ev['status']} in {ev['secs']}s")
    elif t == "notice":
        print(f"== {ev['kind']}: {ev['text']}")
    sys.stdout.flush()


def cmd_goal(rest):
    from .engine import MODES, Engine
    p = argparse.ArgumentParser(prog="antiquota goal", description="Work on an objective until it is verified done.")
    p.add_argument("objective")
    p.add_argument("--cwd", default=".")
    p.add_argument("--model", default="gemini-3.8-flash-high", help="a model id from `agy models`; effort is part of it")
    p.add_argument("--mode", default="accept-edits", choices=list(MODES))
    p.add_argument("--check", default="", help="shell command that must exit 0 when the goal is met")
    p.add_argument("--no-verifier", action="store_true", help="skip the independent verifier")
    p.add_argument("--verifier-model", default="gemini-3.8-flash-high")
    p.add_argument("--max-rounds", type=int, default=20)
    o = p.parse_args(rest)
    eng = Engine()
    meta = eng.create(o.cwd, o.model, "", o.mode, title=o.objective[:60])
    sid, q = meta["id"], eng.subscribe()
    eng.set_goal(sid, o.objective, o.check, not o.no_verifier, o.verifier_model, o.max_rounds)
    s = eng.sessions[sid]
    try:
        while s.busy or not q.empty():
            try:
                ev = q.get(timeout=0.5)
            except queue.Empty:
                continue
            if ev.get("sid") == sid:
                _print_event(ev, meta["cwd"])
    except KeyboardInterrupt:
        eng.stop(sid)
        print(f"\nstopped; session {sid} can be resumed in the GUI")
    g = s.meta["goal"]
    return 0 if g and g["state"] == "met" else 1


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    cmd, rest = (argv[0], argv[1:]) if argv else ("accounts", [])
    if cmd == "run":
        pin = None
        if rest[:1] == ["--slot"] and len(rest) > 1:
            pin, rest = rest[1], rest[2:]
        if rest[:1] == ["--"]:
            rest = rest[1:]
        return a.run(rest, pin)
    if cmd == "gui":
        return cmd_gui(rest)
    if cmd == "serve":  # the engine alone, for the desktop app or a service manager
        return cmd_gui(rest, open_browser=False)
    if cmd == "hook":  # agy runs this (the bundled engine has no separate hook.py)
        from .hook import main as hook_main
        return hook_main(rest)
    if cmd == "goal":
        return cmd_goal(rest)
    if cmd == "login" and not rest:
        return a.login()
    if cmd == "stack":
        return cmd_stack(rest)
    if cmd == "unwall" and len(rest) == 1:
        return cmd_unwall(rest[0])
    if cmd == "probe" and not rest:
        return a.probe()
    if cmd in ("accounts", "doctor") and not rest:
        return cmd_accounts() if cmd == "accounts" else cmd_doctor()
    print(USAGE, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
