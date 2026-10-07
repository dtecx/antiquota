# ADR 0003: Preserve existing files and signed-in accounts

Status: Accepted. The local backup exists; rewrite compatibility proof is pending.

The rewrite reads the user's current data root directly. It retains sessions, stack, slots, settings, permissions, runtime state, worktree records, checkpoints, and the conversation store link described in [DATA_FORMATS.md](../rewrite/DATA_FORMATS.md). There is no automatic relocation or destructive migration.

Typed readers supply legacy missing defaults and retain unknown fields through flattened JSON values, including nested objects. Enable serde_json's `arbitrary_precision` and prove exact opaque integer/decimal retention beyond floating-point precision. Merge updates into the stored object rather than resubmitting opaque values through JavaScript numbers. Existing string-valued settings stay string-valued. Credential files are opaque bytes; agy owns refreshes. Every runner, probe, verifier, login, and transcription uses the selected per-account HOME plus SSH_CONNECTION. Nothing writes the real CLI or IDE configuration.

One engine owns a root at a time. The existing engine/account locks must interoperate with Python on all three platforms. Prefer standard-library file locking, whose current Unix implementation uses flock and whose Windows implementation uses LockFileEx, after cross-language tests. Different data roots and ports support side-by-side development. [Rust file-lock documentation](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock).

The local backup at `~/.antiquota.bak-2026-10-07-152006` preserves five token files byte-for-byte. Private evidence stays ignored and local. Before first live Rust access and after authorized live tests, compare account identity, credential mode, links, and session counts. A rollback stops all writers and preserves new work before any restore; backup restore is never an implicit first response.

Proof: existing fixture reads/writes, unknown-field retention, malformed/interrupted files, Python/Rust lock exclusion, five-account continuity, and a reversible rollback exercise. Any incompatible API or data-format decision remains the user's decision.
