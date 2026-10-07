//! Proves attachments and dictation through isolated real HTTP.
#[path = "../../../tests/rewrite/media.rs"]
mod media;
#[allow(
    dead_code,
    reason = "Shared transport helpers serve multiple integration targets."
)]
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
