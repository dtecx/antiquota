//! Proves Slice 9a through captured HTTP and focused boundary regressions.
#[path = "../../../tests/rewrite/parity.rs"]
mod parity;
#[path = "../../../tests/rewrite/parity_stack.rs"]
mod parity_stack;
#[allow(dead_code, reason = "Shared fixtures also support account contracts.")]
#[path = "../../../tests/rewrite/accounts_setup.rs"]
mod setup;
#[allow(dead_code, reason = "Shared helpers support other contract targets.")]
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
