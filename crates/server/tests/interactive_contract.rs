//! Proves the interactive CLI in a real PTY against isolated HTTP sessions and synthetic hooks.
#[path = "../../../tests/rewrite/interactive.rs"]
mod interactive;
#[allow(
    dead_code,
    reason = "Shared helpers cover several integration targets."
)]
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
