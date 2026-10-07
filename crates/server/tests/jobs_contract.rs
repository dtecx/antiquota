//! Proves background jobs, loops and schedules through isolated real HTTP.
#[path = "../../../tests/rewrite/jobs.rs"]
mod jobs;
#[allow(
    dead_code,
    reason = "Shared transport helpers serve multiple integration targets."
)]
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
