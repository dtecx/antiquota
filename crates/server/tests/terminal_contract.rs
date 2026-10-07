//! Proves the Terminal panel's routes and stream through isolated real HTTP.
#[allow(
    dead_code,
    reason = "Shared transport helpers serve multiple integration targets."
)]
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
#[path = "../../../tests/rewrite/terminal.rs"]
mod terminal;
