//! Proves goal mode, the verifier, denied-command follow-ups and plan mode through real HTTP.
#[path = "../../../tests/rewrite/goals.rs"]
mod goals;
#[allow(
    dead_code,
    reason = "Shared transport helpers serve multiple integration targets."
)]
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
