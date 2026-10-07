"""Git for sessions: a separate worktree per session, and reviewing, committing and merging
what the agent changed. Plain `git` commands; nothing here pushes anywhere."""
import os
import re
import subprocess

ENV = {**os.environ, "GIT_TERMINAL_PROMPT": "0", "GIT_OPTIONAL_LOCKS": "0"}


def git(cwd, *args, check=True, timeout=60):
    r = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, timeout=timeout, env=ENV,
                       stdin=subprocess.DEVNULL)
    if check and r.returncode != 0:
        raise ValueError((r.stderr or r.stdout).strip().splitlines()[-1] if (r.stderr or r.stdout).strip()
                         else f"git {args[0]} failed")
    return r.stdout


def root(cwd):
    """The repository's top folder, or None outside a git repository."""
    try:
        return git(cwd, "rev-parse", "--show-toplevel").strip() or None
    except (ValueError, OSError, subprocess.TimeoutExpired):
        return None


def slug(text):
    return re.sub(r"[^a-z0-9]+", "-", (text or "").lower()).strip("-")[:32] or "session"


def add_worktree(cwd, sid, title, base_dir):
    """A new branch and worktree for a session, starting from the current commit. Returns the
    worktree record and the folder the session works in (same subfolder as `cwd`)."""
    repo = root(cwd)
    if not repo:
        raise ValueError(f"{cwd} is not inside a git repository")
    if not git(repo, "rev-parse", "--verify", "-q", "HEAD", check=False).strip():
        raise ValueError("the repository has no commits yet: commit once, then start a worktree session")
    base = git(repo, "rev-parse", "--abbrev-ref", "HEAD").strip()
    sha = git(repo, "rev-parse", "HEAD").strip()
    branch = f"antiquota/{slug(title)}-{sid[:4]}"
    path = os.path.join(base_dir, f"{os.path.basename(repo)}-{sid}")
    os.makedirs(base_dir, exist_ok=True)
    git(repo, "worktree", "add", "-b", branch, path, sha)
    sub = os.path.relpath(os.path.realpath(cwd), os.path.realpath(repo))
    work = os.path.normpath(os.path.join(path, sub)) if sub != "." else path
    os.makedirs(work, exist_ok=True)
    return {"repo": repo, "path": path, "branch": branch, "base": base, "base_sha": sha}, work


def _numstat(top, *rev):
    out = {}
    for line in git(top, "diff", "--numstat", *rev, check=False).splitlines():
        parts = line.split("\t")
        if len(parts) == 3:
            out[parts[2]] = (int(parts[0]) if parts[0].isdigit() else 0, int(parts[1]) if parts[1].isdigit() else 0)
    return out


def status(cwd, wt=None):
    """Branch, uncommitted files with +/- counts, and for a worktree what its branch adds to the base."""
    top = root(cwd)
    if not top:
        return None
    branch = git(top, "rev-parse", "--abbrev-ref", "HEAD", check=False).strip()
    stats = _numstat(top, "HEAD")
    files = []
    for line in git(top, "status", "--porcelain=v1", "-uall", check=False).splitlines():
        code, path = line[:2], line[3:]
        if " -> " in path:
            path = path.split(" -> ", 1)[1]
        add, rem = stats.get(path, (0, 0))
        if code == "??":
            try:
                with open(os.path.join(top, path), errors="replace") as f:
                    add = sum(1 for _ in f)
            except OSError:
                pass
        files.append({"path": path, "code": code.strip() or "M", "add": add, "del": rem})
    out = {"root": top, "branch": branch, "files": files, "worktree": wt}
    if wt:
        out["ahead"] = int(git(top, "rev-list", "--count", f"{wt['base_sha']}..HEAD", check=False).strip() or 0)
        out["committed"] = [{"path": p, "add": a, "del": d} for p, (a, d) in _numstat(top, f"{wt['base_sha']}..HEAD").items()]
        out["merged"] = out["ahead"] > 0 and subprocess.run(
            ["git", "merge-base", "--is-ancestor", wt["branch"], wt["base"]], cwd=wt["repo"], env=ENV,
            capture_output=True).returncode == 0
    return out


def diff(cwd, path=None, committed=False, wt=None):
    """Unified diff of uncommitted changes (untracked files shown as added), or with `committed`,
    of what the worktree branch adds to its base."""
    top = root(cwd)
    if not top:
        raise ValueError("not a git repository")
    if committed:
        if not wt:
            raise ValueError("only worktree sessions have a base to compare with")
        return git(top, "diff", f"{wt['base_sha']}..HEAD", "--", *([path] if path else []), check=False)
    text = git(top, "diff", "HEAD", "--", *([path] if path else []), check=False)
    untracked = git(top, "ls-files", "--others", "--exclude-standard", "--", *([path] if path else []), check=False)
    for p in untracked.splitlines():
        text += git(top, "diff", "--no-index", "--", os.devnull, p, check=False)
    return text


def commit(cwd, message, paths=None):
    top = root(cwd)
    if not top:
        raise ValueError("not a git repository")
    if not (message or "").strip():
        raise ValueError("write a commit message")
    git(top, "add", "-A", "--", *(paths or ["."]))
    if not git(top, "diff", "--cached", "--name-only", check=False).strip():
        raise ValueError("nothing to commit")
    git(top, "commit", "-q", "-m", message.strip())
    return git(top, "rev-parse", "--short", "HEAD").strip()


def discard(cwd, path):
    """Put one file back as it is in HEAD (a new file is deleted)."""
    top = root(cwd)
    if not top or not path or os.path.isabs(path) or ".." in path.split("/"):
        raise ValueError("pick a file inside the repository")
    tracked = git(top, "ls-files", "--error-unmatch", "--", path, check=False).strip()
    if tracked:
        git(top, "checkout", "HEAD", "--", path)
    else:
        os.remove(os.path.join(top, path))


def merge(wt, message):
    """Merge a worktree branch into its base in the main checkout. Refuses when the main checkout
    is on another branch or has uncommitted changes, so nothing of the user's is touched."""
    repo = wt["repo"]
    if git(repo, "rev-parse", "--abbrev-ref", "HEAD").strip() != wt["base"]:
        raise ValueError(f"the main checkout is not on {wt['base']}; switch it there first")
    if git(repo, "status", "--porcelain", "--untracked-files=no").strip():
        raise ValueError(f"{repo} has uncommitted changes; commit or stash them first")
    if git(wt["path"], "status", "--porcelain").strip():
        raise ValueError("the session has uncommitted changes: commit them first")
    git(repo, "merge", "--no-ff", "-m", message, wt["branch"])
    return git(repo, "rev-parse", "--short", "HEAD").strip()


def remove_worktree(wt, delete_branch=False):
    git(wt["repo"], "worktree", "remove", "--force", wt["path"], check=False)
    git(wt["repo"], "worktree", "prune", check=False)
    if delete_branch:
        git(wt["repo"], "branch", "-D", wt["branch"], check=False)
