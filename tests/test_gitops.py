import os
import subprocess

from antiquota.engine import Engine


def sh(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout


def repo(tmp_path):
    r = tmp_path / "repo"
    (r / "src").mkdir(parents=True)
    sh(r, "init", "-q", "-b", "main")
    sh(r, "config", "user.email", "t@example.com")
    sh(r, "config", "user.name", "T")
    (r / "src" / "app.py").write_text("print(1)\n")
    sh(r, "add", "-A")
    sh(r, "commit", "-qm", "init")
    return r


def test_worktree_session_review_commit_merge(home, tmp_path):
    r = repo(tmp_path)
    eng = Engine()
    m = eng.create(str(r / "src"), "gemini-3.8-flash-low", title="Fix the app", worktree=True)
    wt = m["worktree"]
    assert wt["branch"].startswith("antiquota/fix-the-app-") and wt["base"] == "main"
    assert m["cwd"] == os.path.join(wt["path"], "src") and os.path.isfile(os.path.join(m["cwd"], "app.py"))
    # the agent's edits land in the worktree, not the user's checkout
    with open(os.path.join(m["cwd"], "app.py"), "a") as f:
        f.write("print(2)\n")
    open(os.path.join(m["cwd"], "new.py"), "w").write("x = 1\ny = 2\n")
    st = eng.git_status(m["id"])
    assert {(f["path"], f["code"], f["add"]) for f in st["files"]} == {("src/app.py", "M", 1), ("src/new.py", "??", 2)}
    d = eng.git_diff(m["id"])
    assert "+print(2)" in d and "+y = 2" in d
    assert (r / "src" / "app.py").read_text() == "print(1)\n"
    eng.git_discard(m["id"], "src/new.py")
    assert not os.path.exists(os.path.join(m["cwd"], "new.py"))
    sha = eng.git_commit(m["id"], "Print two")["sha"]
    assert sha and eng.git_status(m["id"])["ahead"] == 1 and eng.git_status(m["id"])["files"] == []
    assert "+print(2)" in eng.git_diff(m["id"], committed=True)
    assert not eng.git_status(m["id"])["merged"]
    eng.git_merge(m["id"])
    assert (r / "src" / "app.py").read_text() == "print(1)\nprint(2)\n"
    assert eng.git_status(m["id"])["merged"]
    meta = eng.git_remove_worktree(m["id"], delete_branch=True)
    assert meta["worktree"] is None and meta["archived"] and not os.path.exists(wt["path"])
    assert wt["branch"] not in sh(r, "branch")


def test_merge_refuses_a_dirty_main_checkout(home, tmp_path):
    r = repo(tmp_path)
    eng = Engine()
    m = eng.create(str(r), "gemini-3.8-flash-low", title="x", worktree=True)
    with open(os.path.join(m["cwd"], "src", "app.py"), "a") as f:
        f.write("print(3)\n")
    eng.git_commit(m["id"], "three")
    (r / "src" / "app.py").write_text("mine\n")  # the user's own uncommitted work
    try:
        eng.git_merge(m["id"])
        assert False
    except ValueError as e:
        assert "uncommitted" in str(e)
    assert (r / "src" / "app.py").read_text() == "mine\n"


def test_worktree_needs_a_repository(home, work):
    try:
        Engine().create(str(work), "gemini-3.8-flash-low", worktree=True)
        assert False
    except ValueError as e:
        assert "not inside a git repository" in str(e)


def test_untitled_worktree_branch_takes_the_first_request(home, tmp_path):
    from conftest import ONE  # noqa: F401  (accounts signed in by the fixture)
    import time
    r = repo(tmp_path)
    eng = Engine()
    m = eng.create(str(r), "gemini-3.8-flash-low", mode="accept-edits", worktree=True)
    assert m["worktree"]["branch"] == f"antiquota/session-{m['id'][:4]}"
    eng.send(m["id"], "Speed up the parser")
    deadline = time.time() + 20
    while eng.sessions[m["id"]].busy and time.time() < deadline:
        time.sleep(0.05)
    wt = eng.sessions[m["id"]].meta["worktree"]
    assert wt["branch"] == f"antiquota/speed-up-the-parser-{m['id'][:4]}"
    assert sh(wt["path"], "rev-parse", "--abbrev-ref", "HEAD").strip() == wt["branch"]
