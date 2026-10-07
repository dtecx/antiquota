# ADR 0002: One typed event stream with compatible JSONL storage

Status: Accepted for the rewrite; recovery tests pending.

Core owns one `Event` enum with the existing `t` discriminant. A record supplies `ts`; SSE supplies the captured nullable `sid`. CLI, UI, and approval services consume these same events. No public wrapper, persisted sequence field, SSE event-name protocol, or replay cursor is added.

Each session retains `events.jsonl` as its append-only log. The store decodes old flat records and preserves extension fields without rewriting them. New metadata transitions use the existing metadata event shape. Recovery starts from the latest complete metadata event, reducing only its subsequent tail. Without one, legacy `meta.json` is authoritative and transcript rows project history without adding usage again; the first Rust metadata event establishes a boundary. New session creation records metadata.

App appends before reducing, replaces the existing snapshot atomically, then publishes. Transcript queries preserve the legacy durable transcript subset even when new logs also retain metadata needed for recovery. Deltas remain typed transient events. Clear/compact keep transcript history; rewind records its notice and changes checkpoints/files. No normal command truncates valid history.

Global usage reconciliation uses the existing fields only: a new ledger row's `t` exactly equals its durable `turn_end.ts`, allocated uniquely per session against event and ledger timestamps. Fixed-clock ties use the next representable finite instant without advancing scheduling time. Reconcile completed records after the first Rust metadata boundary by `(session, t)` under one global writer mutex. Legacy rows remain verbatim, including legitimate identical rows; token/model tuples are never identity. Diagnose partial ledger lines and flush pending usage before session deletion while retaining historical ledger rows.

Interrupted final lines remain locally available for diagnosis and recovery. Slow SSE readers reconnect and reload state instead of silently missing events. Account updates share the Event enum while account state remains in its existing files; they do not create an additional session journal.

Proof: old fixture decoding, property-based replay, crash injection between event/ledger append, snapshot, and publication; exact transcript projection; huge output; slow SSE consumers; equal-token turns at one fake instant; backward wall time/restart timestamp collisions; and session deletion retaining usage history.
