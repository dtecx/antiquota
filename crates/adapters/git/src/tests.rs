//! Proves worktrees, review, commit, discard, merge and removal against real repositories.
use super::*;

struct Repo {
    root: std::path::PathBuf,
}
impl Repo {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "antiquota-git-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("main/sub")).unwrap();
        let repo = Self { root };
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["config", "user.email", "test@example.com"],
            &["config", "user.name", "Test"],
            &["config", "commit.gpgsign", "false"],
        ] {
            git(repo.main(), args).unwrap();
        }
        repo
    }
    fn main(&self) -> &str {
        self.root.join("main").to_str().unwrap().to_owned().leak()
    }
    fn write(&self, dir: &str, name: &str, text: &str) {
        std::fs::write(Path::new(dir).join(name), text).unwrap();
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn worktrees_need_a_commit_and_keep_the_subfolder() {
    let repo = Repo::new("worktree");
    let vcs = Git;
    let folder = repo
        .root
        .join("trees/main-abc")
        .to_string_lossy()
        .into_owned();
    let sub = format!("{}/sub", repo.main());
    let error = vcs
        .add_worktree(&sub, "antiquota/x-abcd", &folder)
        .unwrap_err();
    assert!(error.to_string().contains("no commits yet"));
    repo.write(repo.main(), "a.txt", "one\n");
    git(repo.main(), &["add", "."]).unwrap();
    git(repo.main(), &["commit", "-q", "-m", "first"]).unwrap();
    repo.write(repo.main(), "dirty.txt", "not copied\n");
    let (wt, work) = vcs.add_worktree(&sub, "antiquota/x-abcd", &folder).unwrap();
    assert_eq!(wt.branch, "antiquota/x-abcd");
    assert_eq!(wt.base, "main");
    assert_eq!(wt.base_sha.len(), 40);
    assert!(Path::new(&work).ends_with("main-abc/sub"));
    assert!(
        !Path::new(&folder).join("dirty.txt").exists(),
        "dirty edits stay behind"
    );
    assert!(vcs.rename_branch(&folder, "antiquota/x-abcd", "antiquota/better-abcd"));
    assert!(!vcs.rename_branch(&folder, "antiquota/missing", "antiquota/other"));
    assert_eq!(vcs.root("/"), None);
}

#[test]
fn review_commit_discard_merge_and_remove() {
    let repo = Repo::new("review");
    let vcs = Git;
    repo.write(repo.main(), "a.txt", "one\ntwo\n");
    git(repo.main(), &["add", "."]).unwrap();
    git(repo.main(), &["commit", "-q", "-m", "first"]).unwrap();
    let folder = repo
        .root
        .join("trees/main-rev")
        .to_string_lossy()
        .into_owned();
    let (wt, work) = vcs
        .add_worktree(repo.main(), "antiquota/rev-abcd", &folder)
        .unwrap();
    repo.write(&work, "a.txt", "one\nchanged\nthree\n");
    repo.write(&work, "new.txt", "x\ny\n");
    let status = vcs.status(&work, Some(&wt)).unwrap().unwrap();
    assert_eq!(status["branch"], "antiquota/rev-abcd");
    let files = status["files"].as_array().unwrap();
    assert_eq!(
        files[0],
        json!({"path": "a.txt", "code": "M", "add": 2, "del": 1})
    );
    assert_eq!(
        files[1],
        json!({"path": "new.txt", "code": "??", "add": 2, "del": 0})
    );
    assert_eq!(
        (status["ahead"].clone(), status["merged"].clone()),
        (json!(0), json!(false))
    );
    let diff = vcs.diff(&work, None, None).unwrap();
    assert!(diff.contains("+changed") && diff.contains("+++ b/new.txt"));
    assert!(
        !vcs.diff(&work, Some("new.txt"), None)
            .unwrap()
            .contains("a.txt")
    );
    vcs.discard(&work, "new.txt").unwrap();
    assert!(!Path::new(&work).join("new.txt").exists());
    for bad in ["", "/etc/passwd", "../a.txt", "x/../../a.txt"] {
        assert!(vcs.discard(&work, bad).is_err(), "{bad}");
    }
    assert_eq!(
        vcs.commit(&work, "  ").unwrap_err().to_string(),
        "write a commit message"
    );
    let sha = vcs.commit(&work, "Change a").unwrap();
    assert!(sha.len() >= 7);
    assert_eq!(
        vcs.commit(&work, "again").unwrap_err().to_string(),
        "nothing to commit"
    );
    let committed = vcs.status(&work, Some(&wt)).unwrap().unwrap();
    assert_eq!(committed["ahead"], 1);
    assert_eq!(
        committed["committed"],
        json!([{"path": "a.txt", "add": 2, "del": 1}])
    );
    assert!(vcs.diff(&work, None, Some(&wt)).unwrap().contains("+three"));
    repo.write(repo.main(), "a.txt", "user edit\n");
    let refused = vcs.merge(&wt, "Merge").unwrap_err().to_string();
    assert!(refused.contains("has uncommitted changes"));
    assert_eq!(
        std::fs::read_to_string(Path::new(repo.main()).join("a.txt")).unwrap(),
        "user edit\n",
        "the user's edit is untouched"
    );
    git(repo.main(), &["checkout", "--", "a.txt"]).unwrap();
    let merged = vcs.merge(&wt, "Merge antiquota/rev-abcd").unwrap();
    assert!(merged.len() >= 7);
    assert_eq!(
        vcs.status(&work, Some(&wt)).unwrap().unwrap()["merged"],
        true
    );
    vcs.remove_worktree(&wt, true).unwrap();
    assert!(!Path::new(&folder).exists());
    assert!(
        soft(repo.main(), &["branch", "--list", "antiquota/rev-abcd"])
            .trim()
            .is_empty()
    );
}
