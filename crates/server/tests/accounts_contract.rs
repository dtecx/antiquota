//! Proves account contracts and handoff using isolated real HTTP and fake agy.
#[path = "../../../tests/rewrite/accounts_cli.rs"]
mod cli;
#[path = "../../../tests/rewrite/accounts_cli_lifecycle.rs"]
mod cli_lifecycle;
#[path = "../../../tests/rewrite/accounts_cli_metadata.rs"]
mod cli_metadata;
#[path = "../../../tests/rewrite/accounts_concurrency.rs"]
mod concurrency;
#[path = "../../../tests/rewrite/accounts_defaults.rs"]
mod defaults;
#[path = "../../../tests/rewrite/accounts_deterministic.rs"]
mod deterministic;
#[path = "../../../tests/rewrite/accounts_handoff.rs"]
mod handoff;
#[path = "../../../tests/rewrite/accounts_lifecycle.rs"]
mod lifecycle;
#[path = "../../../tests/rewrite/accounts_metadata_limits.rs"]
mod metadata_limits;
#[path = "../../../tests/rewrite/accounts_models.rs"]
mod models;
#[path = "../../../tests/rewrite/accounts_replay.rs"]
mod replay;
#[path = "../../../tests/rewrite/accounts_rotation.rs"]
mod rotation;
#[path = "../../../tests/rewrite/accounts_setup.rs"]
mod setup;
#[path = "../../../tests/rewrite/storage.rs"]
mod storage;
#[allow(
    dead_code,
    reason = "Shared transport helpers serve multiple integration targets."
)]
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
