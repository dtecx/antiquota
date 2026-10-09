//! Proves the HTTP boundary through isolated real engines with the fake agent.
#[path = "../../../tests/rewrite/http_transport.rs"]
mod http_transport;
#[allow(
    dead_code,
    reason = "Shared transport helpers serve multiple integration targets."
)]
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
