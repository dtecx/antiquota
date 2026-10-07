//! Proves worktree sessions, change review and project files through isolated real HTTP.
#[path = "../../../tests/rewrite/git.rs"]
mod git;
#[allow(
    dead_code,
    reason = "Shared transport helpers serve multiple integration targets."
)]
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
