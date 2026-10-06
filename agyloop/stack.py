"""agy-loop's own agy config: inspect and edit it, or copy items in from your
agy CLI (~/.gemini) or the Antigravity IDE (~/.gemini/antigravity).

Those two folders are only ever read. Everything written lands in the stack
(<base>/stack/.gemini), which every agy-loop account links to.
"""
import hashlib
import json
import os
import re
import shutil
import subprocess

from . import accounts as a

# Where each kind of item lives under each root. Missing source = that source has none.
DIRS = {"skill": {"loop": "config/skills", "cli": "config/skills", "ide": "skills"},
        "plugin": {"loop": "config/plugins", "cli": "config/plugins"}}
FILES = {"rule": {"loop": "config/rules", "cli": "config/rules"}}
ENTRIES = {"hook": {"loop": ("config/hooks.json", None), "cli": ("config/hooks.json", None)},
           "mcp": {"loop": ("config/mcp_config.json", "mcpServers"), "cli": ("config/mcp_config.json", "mcpServers"),
                   "ide": ("mcp_config.json", "mcpServers")}}
KINDS = {**DIRS, **FILES, **ENTRIES}
HOOK_EVENTS = ("PreToolUse", "PostToolUse", "PreInvocation", "PostInvocation", "Stop")
NAME = re.compile(r"[A-Za-z0-9._:@+-]{1,100}")
NOT_EDITABLE = {*a.CLONE_SKIP["antigravity-cli"], "log", "cache", "crashes", "presence", "updater"}


def roots():
    return {"loop": os.path.join(a.stack_home(), ".gemini"), "cli": os.path.join(a.home(), ".gemini"),
            "ide": os.path.join(a.home(), ".gemini/antigravity")}


def _check(kind, name=None, source=None):
    if kind not in KINDS:
        raise ValueError(f"unknown kind: {kind}")
    if source is not None and source not in KINDS[kind]:
        raise ValueError(f"{source} has no {kind}s")
    if name is not None and (not NAME.fullmatch(name) or name.startswith(".")):
        raise ValueError(f"bad name: {name!r}")


def _load(path):
    try:
        with open(path) as f:
            d = json.load(f)
        return d if isinstance(d, dict) else {}
    except (OSError, ValueError):
        return {}


def _save(path, d):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path + ".tmp", "w") as f:
        json.dump(d, f, indent=2)
        f.write("\n")
    os.replace(path + ".tmp", path)


def _digest(path):
    h = hashlib.sha1()
    if os.path.isfile(path):
        with open(path, "rb") as f:
            h.update(f.read())
        return h.hexdigest()
    if not os.path.isdir(path):
        return None
    for d, dirs, files in sorted(os.walk(path)):
        dirs.sort()
        for n in sorted(files):
            p = os.path.join(d, n)
            h.update(os.path.relpath(p, path).encode())
            try:
                with open(p, "rb") as f:
                    h.update(f.read())
            except OSError:
                pass
    return h.hexdigest()


def _entries_file(kind, src):
    rel, key = ENTRIES[kind][src]
    path = os.path.join(roots()[src], rel)
    d = _load(path)
    return path, d, (d.setdefault(key, {}) if key else d)


def _items(kind, src):
    """{name: comparable value} for one kind at one root. JSON entries compare after path rewriting."""
    root = roots()[src]
    if kind in DIRS or kind in FILES:
        d = os.path.join(root, KINDS[kind][src])
        names = [n for n in (os.listdir(d) if os.path.isdir(d) else []) if not n.startswith(".")]
        if kind == "skill":
            names = [n for n in names if os.path.isfile(os.path.join(d, n, "SKILL.md"))]
        return {n: _digest(os.path.join(d, n)) for n in names}
    _, _, entries = _entries_file(kind, src)
    fix = a.to_stack_paths if src != "loop" else (lambda t: t)
    return {n: fix(json.dumps(v, sort_keys=True)) for n, v in entries.items()
            if isinstance(v, dict) and not (kind == "hook" and n.startswith("$"))}


def agy_in_stack(*args):
    """Run an agy subcommand against agy-loop's stack (never against your own ~/.gemini)."""
    h = a.probe_home()
    a.sync(h)
    cmd, env = a.agy_cmd(h, list(args))
    return subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=60, stdin=subprocess.DEVNULL)


def _mcp_status():
    try:
        lines = agy_in_stack("mcp", "list").stdout.splitlines()[1:]
    except (OSError, subprocess.TimeoutExpired):
        return {}
    return {p[0]: p[2] for p in (ln.split() for ln in lines) if len(p) >= 3}


def _describe(kind, name, value_src):
    """Short human summary of an item, read from wherever it exists."""
    src, root = value_src, roots()[value_src]
    if kind == "skill":
        try:
            with open(os.path.join(root, KINDS[kind][src], name, "SKILL.md")) as f:
                m = re.search(r"^description:\s*(.+)$", f.read(4000), re.M)
            return m.group(1).strip().strip("\"'")[:200] if m else ""
        except OSError:
            return ""
    if kind in ENTRIES:
        _, _, entries = _entries_file(kind, src)
        v = entries.get(name) or {}
        if kind == "hook":
            return ", ".join(e for e in HOOK_EVENTS if e in v)
        return v.get("url") or v.get("serverUrl") or " ".join([v.get("command", ""), *map(str, v.get("args", []))]).strip()
    return ""


def overview():
    a.seed_stack()
    R = roots()
    out = {"path": R["loop"]}
    mcp_state = _mcp_status()
    hooks_loop = _entries_file("hook", "loop")[2]
    plugins_cfg = _load(os.path.join(R["loop"], "config/config.json")).get("plugins", {})
    for kind, spec in KINDS.items():
        loop = _items(kind, "loop")
        other = {s: _items(kind, s) for s in spec if s != "loop"}
        names = sorted(set(loop).union(*other.values()), key=str.lower)
        rows = []
        for n in names:
            row = {"name": n, "here": n in loop,
                   "about": _describe(kind, n, "loop" if n in loop else next(s for s in other if n in other[s]))}
            for s, items in other.items():
                row[s] = None if n not in items else ("same" if items[n] == loop.get(n) else "differs")
            if kind == "hook" and n in loop:
                row["enabled"] = hooks_loop[n].get("enabled", True) is not False
            if kind == "mcp" and n in loop:
                row["enabled"] = mcp_state.get(n, "enabled") == "enabled"
            if kind == "plugin" and n in loop:
                p = plugins_cfg.get(n)
                row["enabled"] = not (isinstance(p, dict) and p.get("enabled") is False)
            rows.append(row)
        out[kind + "s"] = rows
    gm = lambda r: _digest(os.path.join(r, "GEMINI.md"))
    out["gemini_md"] = {"cli": None if gm(R["cli"]) is None else ("same" if gm(R["cli"]) == gm(R["loop"]) else "differs")}
    perms = lambda r: _load(os.path.join(r, "antigravity-cli/settings.json")).get("permissions", {})
    out["permissions"] = {"allow": perms(R["loop"]).get("allow", []), "deny": perms(R["loop"]).get("deny", []),
                          "cli": "same" if perms(R["cli"]) == perms(R["loop"]) else "differs"}
    return out


def copy(source, kind, name=None):
    """Copy one item (or every item of a kind) from your agy CLI or the IDE into the stack."""
    R = roots()
    if kind == "everything":
        if source != "cli":
            raise ValueError("everything comes from the agy CLI; copy IDE items one kind at a time")
        a.clone_stack()
        return
    if kind == "gemini_md" or kind == "permissions":
        if source != "cli":
            raise ValueError(f"{kind} comes from the agy CLI")
        if kind == "gemini_md":
            a._replace(os.path.join(R["cli"], "GEMINI.md"), os.path.join(R["loop"], "GEMINI.md"))
        else:
            dst = os.path.join(R["loop"], "antigravity-cli/settings.json")
            d = _load(dst)
            d["permissions"] = _load(os.path.join(R["cli"], "antigravity-cli/settings.json")).get("permissions", {})
            _save(dst, d)
        return
    _check(kind, name, source)
    names = [name] if name else list(_items(kind, source))
    if kind in DIRS or kind in FILES:
        for n in names:
            src = os.path.join(R[source], KINDS[kind][source], n)
            if not os.path.exists(src):
                raise KeyError(n)
            a._replace(src, os.path.join(R["loop"], KINDS[kind]["loop"], n))
            if kind == "plugin":
                data = os.path.join(R[source], "antigravity-cli/plugin_data", n)
                if os.path.isdir(data):
                    a._replace(data, os.path.join(R["loop"], "antigravity-cli/plugin_data", n))
                cfg_src = _load(os.path.join(R[source], "config/config.json")).get("plugins", {})
                if n in cfg_src:
                    p = os.path.join(R["loop"], "config/config.json")
                    d = _load(p)
                    d.setdefault("plugins", {})[n] = cfg_src[n]
                    _save(p, d)
        return
    _, _, src_entries = _entries_file(kind, source)
    path, d, dst_entries = _entries_file(kind, "loop")
    for n in names:
        if n not in src_entries:
            raise KeyError(n)
        dst_entries[n] = json.loads(a.to_stack_paths(json.dumps(src_entries[n])))
        state = os.path.join(R[source], "antigravity-cli/mcp" if source == "cli" else "mcp", n)
        if kind == "mcp" and os.path.isdir(state) and not os.path.exists(os.path.join(R["loop"], "antigravity-cli/mcp", n)):
            a._replace(state, os.path.join(R["loop"], "antigravity-cli/mcp", n))
    _save(path, d)
    if kind == "hook":  # hook commands point at scripts in config/hooks; bring them along
        scripts = os.path.join(R[source], "config/hooks")
        if os.path.isdir(scripts):
            shutil.copytree(scripts, os.path.join(R["loop"], "config/hooks"), dirs_exist_ok=True)


def delete(kind, name):
    _check(kind, name)
    R = roots()
    if kind in DIRS or kind in FILES:
        p = os.path.join(R["loop"], KINDS[kind]["loop"], name)
        if not os.path.lexists(p):
            raise KeyError(name)
        shutil.rmtree(p) if os.path.isdir(p) and not os.path.islink(p) else os.remove(p)
        if kind == "plugin":
            shutil.rmtree(os.path.join(R["loop"], "antigravity-cli/plugin_data", name), ignore_errors=True)
        return
    path, d, entries = _entries_file(kind, "loop")
    if name not in entries:
        raise KeyError(name)
    del entries[name]
    _save(path, d)


def get_item(kind, name):
    _check(kind, name)
    if kind not in ENTRIES:
        raise ValueError("only hooks and MCP servers are JSON items; edit files instead")
    return _entries_file(kind, "loop")[2][name]


def put_item(kind, name, value):
    _check(kind, name)
    if kind not in ENTRIES or not isinstance(value, dict):
        raise ValueError("a hook or MCP server is a JSON object")
    path, d, entries = _entries_file(kind, "loop")
    entries[name] = value
    _save(path, d)


def set_enabled(kind, name, on):
    _check(kind, name)
    if kind == "hook":
        path, d, entries = _entries_file(kind, "loop")
        if name not in entries:
            raise KeyError(name)
        entries[name]["enabled"] = bool(on)
        _save(path, d)
    elif kind in ("mcp", "plugin"):
        r = agy_in_stack(kind, "enable" if on else "disable", name)
        if r.returncode != 0:
            raise ValueError((r.stdout + r.stderr).strip()[-300:] or f"agy {kind} {'enable' if on else 'disable'} failed")
    else:
        raise ValueError(f"{kind}s cannot be switched off; delete them instead")


def _editable(rel):
    root = os.path.realpath(roots()["loop"])
    p = os.path.realpath(os.path.join(root, rel or ""))
    parts = os.path.relpath(p, root).split(os.sep)
    if not p.startswith(root + os.sep) or parts[0] == ".." or (
            len(parts) > 1 and parts[0] == "antigravity-cli" and parts[1] in NOT_EDITABLE):
        raise ValueError(f"not an editable stack file: {rel}")
    return p


def read_file(rel):
    p = _editable(rel)
    if not os.path.isfile(p):
        return {"path": rel, "text": "", "exists": False}
    if os.path.getsize(p) > 1_000_000:
        raise ValueError("file too large to edit here")
    with open(p, encoding="utf-8", errors="replace") as f:
        return {"path": rel, "text": f.read(), "exists": True}


def write_file(rel, text):
    p = _editable(rel)
    if not isinstance(text, str) or len(text) > 1_000_000:
        raise ValueError("text too large")
    if p.endswith(".json"):
        try:
            json.loads(text)
        except ValueError as e:
            raise ValueError(f"invalid JSON: {e}")
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p + ".tmp", "w", encoding="utf-8") as f:
        f.write(text)
    os.replace(p + ".tmp", p)


# --- agy-loop's own GUI defaults --------------------------------------------------

DEFAULTS = {"model": "gemini-3.8-flash-high", "mode": "default", "verifier_model": "claude-sonnet-4-6"}


def defaults():
    return {**DEFAULTS, **{k: v for k, v in _load(os.path.join(a.base(), "settings.json")).items() if k in DEFAULTS}}


def set_defaults(values):
    d = {**defaults(), **{k: str(v) for k, v in (values or {}).items() if k in DEFAULTS}}
    _save(os.path.join(a.base(), "settings.json"), d)
    return d
