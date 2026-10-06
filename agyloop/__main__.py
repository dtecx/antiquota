import json
import os
import sys
from datetime import datetime

from . import accounts as a

USAGE = """usage: agyloop <command>

  accounts             signed-in accounts, quota and walls (default)
  login <name>         sign a new Google account into slot <name>
  probe                read /quota for every account and wall the spent ones
  doctor               check that every account shares the same stack
  unwall <name|all>    clear walls by hand
  run [--slot S] [--] <agy args...>
                       run agy on the first account with quota left;
                       exit 75 when every account is out of quota"""


def cmd_accounts():
    rows = a.slots()
    if not rows:
        print("no signed-in account; sign in to agy or run: agyloop login <name>")
        return 0
    print(f"{'SLOT':10} {'GEMINI':30} {'CLAUDE/GPT':30} LEASE")
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
        print(f"{s:10} {cells[0]:30} {cells[1]:30} {'free' if fd is not None else 'busy'}")
    return 0


def cmd_doctor():
    h = a.home()
    cfg = os.path.join(h, ".gemini/config")

    def names(p):
        return sorted(n for n in os.listdir(p) if not n.startswith(".")) if os.path.isdir(p) else []

    try:
        with open(os.path.join(cfg, "mcp_config.json")) as f:
            mcp = sorted(json.load(f).get("mcpServers", {}))
    except (OSError, ValueError):
        mcp = []
    ref = a.loaded("main")
    print("stack as agy reports it for main:")
    print(f"  skills ({len(ref['skills'] or [])}): {', '.join(ref['skills'] or ['<unreadable>'])}")
    print(f"  hooks:   {', '.join(ref['hooks'] or ['<unreadable>'])}")
    print(f"  mcp:     {', '.join(mcp)}")
    ide_only = sorted(set(names(os.path.join(h, ".gemini/antigravity/skills"))) - set(ref["skills"] or []))
    if ide_only:
        print(f"  warn: {len(ide_only)} skills live only in ~/.gemini/antigravity/skills, "
              f"which agy CLI does not load: {', '.join(ide_only)}")

    problems = 0
    main_auth = os.path.join(h, a.AUTH)
    for s in a.slots():
        if s == "main":
            continue
        sh = a.slot_home(s)
        a.sync(sh)
        bad = a.unshared(sh)
        auth = os.path.join(sh, a.AUTH)
        if os.path.islink(auth) or (os.path.exists(main_auth) and os.path.samefile(auth, main_auth)):
            bad.append(a.AUTH + " (must be private)")
        got = a.loaded(s)
        bad += [f"agy {k} differ" for k in ref if got[k] != ref[k]]
        print(f"{s}: " + ("ok, agy loads the same skills and hooks, auth private" if not bad
                          else "PROBLEM: " + ", ".join(bad)))
        problems += len(bad)
    return 1 if problems else 0


def cmd_unwall(target):
    for s in (a.slots() if target == "all" else [target]):
        for b in a.BUCKETS:
            a.clear_wall(s, b)
    print(f"walls cleared: {target}")
    return 0


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
    if cmd == "login" and len(rest) == 1:
        return a.login(rest[0])
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
