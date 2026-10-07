//! Owns a session's git review and worktree lifecycle, and read-only project file views.
use crate::{AppError, Service};
use antiquota_core::worktree::{Worktree, branch_name};
use serde_json::{Value, json};

impl Service {
    fn worktree_of(&self, id: &str) -> Result<(String, Option<Worktree>), AppError> {
        let meta = self.get(id)?.0;
        let worktree = meta.worktree.as_ref().and_then(Worktree::from_value);
        Ok((meta.cwd, worktree))
    }

    /// A new worktree for a session that is being created: a branch named after its title
    /// and a folder named after the repository and session.
    pub(crate) fn new_worktree(
        &self,
        cwd: &str,
        sid: &str,
        title: &str,
    ) -> Result<(Worktree, String), AppError> {
        let repo = self
            .vcs
            .root(cwd)
            .ok_or_else(|| AppError::Invalid(format!("{cwd} is not inside a git repository")))?;
        let name = std::path::Path::new(&repo)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "repo".into());
        let base = self.store.worktrees_dir();
        if base.is_empty() {
            return Err(AppError::Invalid("worktrees are unavailable".into()));
        }
        let folder = std::path::Path::new(&base).join(format!("{name}-{sid}"));
        Ok(self
            .vcs
            .add_worktree(cwd, &branch_name(title, sid), &folder.to_string_lossy())?)
    }

    /// Renames a worktree branch made before the session had a title, once it has one.
    pub(crate) fn name_branch(&self, state: &mut crate::SessionState) -> Result<(), AppError> {
        let Some(mut worktree) = state.meta.worktree.as_ref().and_then(Worktree::from_value) else {
            return Ok(());
        };
        let id = state.meta.id.clone();
        if worktree.branch != branch_name("", &id) || state.meta.title.is_empty() {
            return Ok(());
        }
        let renamed = branch_name(&state.meta.title, &id);
        if renamed != worktree.branch
            && self
                .vcs
                .rename_branch(&worktree.path, &worktree.branch, &renamed)
        {
            worktree.branch = renamed;
            let mut next = state.meta.clone();
            next.worktree = Some(worktree.to_value());
            self.replace_meta(state, next)?;
        }
        Ok(())
    }

    /// Branch and changed files, with what a worktree adds to its base; `{}` outside git.
    pub fn git_status(&self, id: &str) -> Result<Value, AppError> {
        let (cwd, worktree) = self.worktree_of(id)?;
        Ok(self
            .vcs
            .status(&cwd, worktree.as_ref())?
            .unwrap_or_else(|| json!({})))
    }

    pub fn git_diff(
        &self,
        id: &str,
        path: Option<&str>,
        committed: bool,
    ) -> Result<String, AppError> {
        let (cwd, worktree) = self.worktree_of(id)?;
        let base = if committed {
            Some(worktree.ok_or_else(|| {
                AppError::Invalid("only worktree sessions have a base to compare with".into())
            })?)
        } else {
            None
        };
        Ok(self.vcs.diff(&cwd, path, base.as_ref())?)
    }

    pub fn git_commit(&self, id: &str, message: &str) -> Result<String, AppError> {
        let (cwd, _) = self.worktree_of(id)?;
        let sha = self.vcs.commit(&cwd, message)?;
        let first: String = message
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(120)
            .collect();
        self.notice(id, "info", &format!("Committed {sha}: {first}"), json!({}))?;
        Ok(sha)
    }

    pub fn git_discard(&self, id: &str, path: &str) -> Result<(), AppError> {
        let (cwd, _) = self.worktree_of(id)?;
        self.vcs.discard(&cwd, path)?;
        self.notice(
            id,
            "info",
            &format!("Discarded the changes to {path}."),
            json!({}),
        )?;
        Ok(())
    }

    pub fn git_merge(&self, id: &str) -> Result<String, AppError> {
        let (session, worktree) = (self.get(id)?.0, self.worktree_of(id)?.1);
        let worktree = worktree
            .ok_or_else(|| AppError::Invalid("this session has no worktree to merge".into()))?;
        let title = if session.title.is_empty() {
            "antiquota session"
        } else {
            &session.title
        };
        let sha = self
            .vcs
            .merge(&worktree, &format!("Merge {}: {title}", worktree.branch))?;
        self.notice(
            id,
            "info",
            &format!("Merged {} into {} ({sha}).", worktree.branch, worktree.base),
            json!({}),
        )?;
        Ok(sha)
    }

    /// Removes the session's worktree (optionally its branch), moves the session back to the
    /// main checkout with a fresh conversation, archives it and tells agy on its next turn.
    pub fn git_remove_worktree(
        &self,
        id: &str,
        delete_branch: bool,
    ) -> Result<antiquota_core::Session, AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        let worktree = state
            .meta
            .worktree
            .as_ref()
            .and_then(Worktree::from_value)
            .ok_or_else(|| AppError::Invalid("this session has no worktree".into()))?;
        if state.busy {
            return Err(AppError::Invalid("stop the running turn first".into()));
        }
        self.vcs.remove_worktree(&worktree, delete_branch)?;
        let mut next = state.meta.clone();
        next.worktree = None;
        next.cwd = worktree.repo.clone();
        next.conversation_id = None;
        next.archived = true;
        next.pending_note =
            "This session's worktree was removed; you now work in the main checkout.".into();
        self.replace_meta(&mut state, next)?;
        let text = format!(
            "Removed the worktree{}; the session is archived.",
            if delete_branch { " and its branch" } else { "" }
        );
        self.append(
            &mut state,
            antiquota_core::Event::Notice {
                kind: "info".into(),
                text,
                data: json!({}),
            },
        )?;
        Ok(state.meta.clone())
    }

    pub fn project_tree(&self, id: &str, path: &str) -> Result<Value, AppError> {
        let cwd = self.get(id)?.0.cwd;
        Ok(self.workspace.tree(&cwd, path)?)
    }

    pub fn read_file(&self, id: &str, path: &str) -> Result<Value, AppError> {
        let cwd = self.get(id)?.0.cwd;
        let attachments = self.store.attachments_dir(id);
        let roots: Vec<String> = [attachments]
            .into_iter()
            .filter(|r| !r.is_empty())
            .collect();
        Ok(self.workspace.read_text(&cwd, &roots, path)?)
    }
}
