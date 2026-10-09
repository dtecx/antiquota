# ADR 0027: Architecture rules as tests

Status: Accepted for Slice 16.

The rewrite requirements set rules for the code itself: the crate layout, a core without input and output, one event enum, no unsafe code, `thiserror` in libraries and `anyhow` only in executables, `tracing` and serde, small modules that state what they own, tests in every crate, and an architecture document with a diagram. `tests/rewrite/architecture.rs` checks each rule against `cargo metadata` and the source text, so a later change that breaks one fails `just test` instead of drifting.

The checks read text and do not parse Rust. They leave out test files and inline test modules, and they read "modules near or below 400 lines" as no product module over 500 lines. The 723-line HTTP module was split: `http.rs` keeps the security checks, uploads and response helpers, and `session_http.rs` holds the session routes.

The CLI `goal` command now decodes the shared `StreamEvent` type. When the engine ends a stream that fell behind, the command opens a new one and shows the saved events it missed from the session snapshot. Durable events have unique timestamps within a session, so each is shown once.
