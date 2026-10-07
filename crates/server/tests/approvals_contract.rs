//! Proves permission rules, tool approvals, the hook and steering through isolated real HTTP.
#[path = "../../../tests/rewrite/approvals.rs"]
mod approvals;
#[allow(
    dead_code,
    reason = "Shared transport helpers serve multiple integration targets."
)]
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
