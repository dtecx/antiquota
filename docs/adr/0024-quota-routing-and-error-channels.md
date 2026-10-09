# ADR 0024: Quota routing and error channels

Status: Accepted for Slice 13.

Routing decisions stay pure functions in `antiquota_core::routing`: the model bucket, headroom, walls from a quota panel, ranking for each rotation, the next reset and error classification. The application layer only reads accounts and history through ports and writes walls and routing entries. Tests use a fixed clock, so wall times and routing timestamps are compared exactly.

Only agy's error channels can wall or park an account. These are stderr of a failed call, error results and agy's own error frames, always without their `response` field. The model's reply never counts. `error_text` is shared by the raw CLI and the separate ask call (verifier and dictation), and the stream decoder applies the same rule to its frames.

`refresh_accounts` returns the accounts that got a valid new reading, like the legacy per-account result. Routing reads without a count return the newest 200 entries; the HTTP default stays 60, and an empty `n` counts as missing.
