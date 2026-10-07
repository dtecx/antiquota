//! Owns git for sessions through the `git` command: worktrees, status, diffs, commits, discards,
//! merges and removal. Credential prompts and optional locks are off; nothing pushes.
use antiquota_core::{PortError, Vcs, worktree::Worktree};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Component, Path},
    process::{Command, Stdio},
};

#[derive(Debug, Default, Clone)]
pub struct Git;

struct Output {
    ok: bool,
    stdout: String,
    stderr: String,
}

fn run(cwd: &str, args: &[&str]) -> Result<Output, PortError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| PortError::Unavailable(format!("git is unavailable: {e}")))?;
    Ok(Output {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Runs git and fails with its last error line.
fn git(cwd: &str, args: &[&str]) -> Result<String, PortError> {
    let out = run(cwd, args)?;
    if out.ok {
        return Ok(out.stdout);
    }
    let text = if out.stderr.trim().is_empty() {
        out.stdout
    } else {
        out.stderr
    };
    let line = text
        .trim()
        .lines()
        .last()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("git {} failed", args.first().unwrap_or(&"")));
    Err(PortError::Invalid(line))
}

/// Runs git and keeps whatever it printed, failure or not.
fn soft(cwd: &str, args: &[&str]) -> String {
    run(cwd, args).map(|o| o.stdout).unwrap_or_default()
}

fn numstat(top: &str, args: &[&str]) -> BTreeMap<String, (u64, u64)> {
    let mut all = vec!["diff", "--numstat"];
    all.extend_from_slice(args);
    soft(top, &all)
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let (add, del, path) = (parts.next()?, parts.next()?, parts.next()?);
            Some((
                path.to_owned(),
                (add.parse().unwrap_or(0), del.parse().unwrap_or(0)),
            ))
        })
        .collect()
}

fn safe_relative(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && !p.is_absolute()
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

impl Vcs for Git {
    fn root(&self, cwd: &str) -> Option<String> {
        git(cwd, &["rev-parse", "--show-toplevel"])
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }

    fn add_worktree(
        &self,
        cwd: &str,
        branch: &str,
        folder: &str,
    ) -> Result<(Worktree, String), PortError> {
        let repo = self
            .root(cwd)
            .ok_or_else(|| PortError::Invalid(format!("{cwd} is not inside a git repository")))?;
        if soft(&repo, &["rev-parse", "--verify", "-q", "HEAD"])
            .trim()
            .is_empty()
        {
            return Err(PortError::Invalid(
                "the repository has no commits yet: commit once, then start a worktree session"
                    .into(),
            ));
        }
        let base = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"])?
            .trim()
            .to_owned();
        let sha = git(&repo, &["rev-parse", "HEAD"])?.trim().to_owned();
        if let Some(parent) = Path::new(folder).parent() {
            std::fs::create_dir_all(parent).map_err(|e| PortError::Unavailable(e.to_string()))?;
        }
        git(&repo, &["worktree", "add", "-b", branch, folder, &sha])?;
        let real = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| p.into());
        let sub = real(cwd)
            .strip_prefix(real(&repo))
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let work = if sub.as_os_str().is_empty() {
            Path::new(folder).to_path_buf()
        } else {
            Path::new(folder).join(&sub)
        };
        std::fs::create_dir_all(&work).map_err(|e| PortError::Unavailable(e.to_string()))?;
        Ok((
            Worktree {
                repo,
                path: folder.into(),
                branch: branch.into(),
                base,
                base_sha: sha,
                extra: Default::default(),
            },
            work.to_string_lossy().into_owned(),
        ))
    }

    fn status(&self, cwd: &str, worktree: Option<&Worktree>) -> Result<Option<Value>, PortError> {
        let Some(top) = self.root(cwd) else {
            return Ok(None);
        };
        let branch = soft(&top, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_owned();
        let stats = numstat(&top, &["HEAD"]);
        let mut files = vec![];
        for line in soft(&top, &["status", "--porcelain=v1", "-uall"]).lines() {
            if line.len() < 4 {
                continue;
            }
            let code = line[..2].trim();
            let mut path = line[3..].to_owned();
            if let Some((_, to)) = path.split_once(" -> ") {
                path = to.to_owned();
            }
            let (mut add, del) = stats.get(&path).copied().unwrap_or((0, 0));
            if code == "??" {
                add = std::fs::read(Path::new(&top).join(&path))
                    .map(|b| String::from_utf8_lossy(&b).lines().count() as u64)
                    .unwrap_or(0);
            }
            files.push(json!({"path": path, "code": if code.is_empty() { "M" } else { code }, "add": add, "del": del}));
        }
        let mut out = json!({
            "root": top,
            "branch": branch,
            "files": files,
            "worktree": worktree.map(Worktree::to_value),
        });
        if let Some(wt) = worktree {
            let range = format!("{}..HEAD", wt.base_sha);
            let ahead: u64 = soft(&top, &["rev-list", "--count", &range])
                .trim()
                .parse()
                .unwrap_or(0);
            let committed: Vec<Value> = numstat(&top, &[&range])
                .into_iter()
                .map(|(path, (add, del))| json!({"path": path, "add": add, "del": del}))
                .collect();
            let merged = ahead > 0
                && run(
                    &wt.repo,
                    &["merge-base", "--is-ancestor", &wt.branch, &wt.base],
                )
                .map(|o| o.ok)
                .unwrap_or(false);
            out["ahead"] = json!(ahead);
            out["committed"] = json!(committed);
            out["merged"] = json!(merged);
        }
        Ok(Some(out))
    }

    fn diff(
        &self,
        cwd: &str,
        path: Option<&str>,
        committed: Option<&Worktree>,
    ) -> Result<String, PortError> {
        let top = self
            .root(cwd)
            .ok_or_else(|| PortError::Invalid("not a git repository".into()))?;
        let narrow: Vec<&str> = path.into_iter().collect();
        if let Some(wt) = committed {
            let range = format!("{}..HEAD", wt.base_sha);
            let mut args = vec!["diff", range.as_str(), "--"];
            args.extend(&narrow);
            return Ok(soft(&top, &args));
        }
        let mut args = vec!["diff", "HEAD", "--"];
        args.extend(&narrow);
        let mut text = soft(&top, &args);
        let mut others = vec!["ls-files", "--others", "--exclude-standard", "--"];
        others.extend(&narrow);
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        for file in soft(&top, &others).lines() {
            text.push_str(&soft(&top, &["diff", "--no-index", "--", null, file]));
        }
        Ok(text)
    }

    fn commit(&self, cwd: &str, message: &str) -> Result<String, PortError> {
        let top = self
            .root(cwd)
            .ok_or_else(|| PortError::Invalid("not a git repository".into()))?;
        let message = message.trim();
        if message.is_empty() {
            return Err(PortError::Invalid("write a commit message".into()));
        }
        git(&top, &["add", "-A", "--", "."])?;
        if soft(&top, &["diff", "--cached", "--name-only"])
            .trim()
            .is_empty()
        {
            return Err(PortError::Invalid("nothing to commit".into()));
        }
        git(&top, &["commit", "-q", "-m", message])?;
        Ok(git(&top, &["rev-parse", "--short", "HEAD"])?
            .trim()
            .to_owned())
    }

    fn discard(&self, cwd: &str, path: &str) -> Result<(), PortError> {
        let top = self
            .root(cwd)
            .filter(|_| safe_relative(path))
            .ok_or_else(|| PortError::Invalid("pick a file inside the repository".into()))?;
        let tracked = soft(&top, &["ls-files", "--error-unmatch", "--", path]);
        if tracked.trim().is_empty() {
            std::fs::remove_file(Path::new(&top).join(path))
                .map_err(|e| PortError::Invalid(e.to_string()))
        } else {
            git(&top, &["checkout", "HEAD", "--", path]).map(|_| ())
        }
    }

    fn merge(&self, wt: &Worktree, message: &str) -> Result<String, PortError> {
        let current = git(&wt.repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        if current.trim() != wt.base {
            return Err(PortError::Invalid(format!(
                "the main checkout is not on {}; switch it there first",
                wt.base
            )));
        }
        if !git(&wt.repo, &["status", "--porcelain", "--untracked-files=no"])?
            .trim()
            .is_empty()
        {
            return Err(PortError::Invalid(format!(
                "{} has uncommitted changes; commit or stash them first",
                wt.repo
            )));
        }
        if !git(&wt.path, &["status", "--porcelain"])?.trim().is_empty() {
            return Err(PortError::Invalid(
                "the session has uncommitted changes: commit them first".into(),
            ));
        }
        git(&wt.repo, &["merge", "--no-ff", "-m", message, &wt.branch])?;
        Ok(git(&wt.repo, &["rev-parse", "--short", "HEAD"])?
            .trim()
            .to_owned())
    }

    fn remove_worktree(&self, wt: &Worktree, delete_branch: bool) -> Result<(), PortError> {
        soft(&wt.repo, &["worktree", "remove", "--force", &wt.path]);
        soft(&wt.repo, &["worktree", "prune"]);
        if delete_branch {
            soft(&wt.repo, &["branch", "-D", &wt.branch]);
        }
        Ok(())
    }

    fn rename_branch(&self, path: &str, from: &str, to: &str) -> bool {
        run(path, &["branch", "-m", from, to]).is_ok_and(|o| o.ok)
            && soft(path, &["rev-parse", "--abbrev-ref", "HEAD"]).trim() == to
    }
}

#[cfg(test)]
mod tests;
