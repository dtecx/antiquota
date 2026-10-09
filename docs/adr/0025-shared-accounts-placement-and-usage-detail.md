# ADR 0025: Shared accounts, placement and usage detail

Status: Accepted for Slice 14, on the owner's request of 2026-10-09.

Legacy antiquota ran one agy call per account at a time, because concurrent agy processes on one home can race the token refresh. The owner asked for several active sessions on one account. Model calls (turns, the verifier, dictation and raw runs) now take a shared lease on the account's lock file; probes, checks, doctor and removal still take it alone. An account reads busy while any lease is held, a probe skips it, and the legacy exclusive lock still excludes a shared one. agy owns its token file, so a concurrent refresh remains a risk to watch in live use (R-011).

A `placement` setting chooses where new sessions go. Fill first, the default, ranks accounts as before and waits briefly for the first one when a probe holds it, so new sessions stay on one account until it is walled or under the reserve. Spread first adds one key after the reserve and the session's own account: accounts without a running call come first. Both are pure ranking rules in core. The default is left out of the settings JSON so captured responses keep their exact keys.

The decoder sums agy's `cache_read_tokens`, the ledger stores it as `cache` when above zero, and `GET /api/usage?detail=1` splits every total into input, cached input, output and thinking. The UI derives the cache hit rate as cached input over all input. The Usage view reads the ledger again shortly after an `accounts` event and every minute while visible; these quiet reads never replace a period change in progress.
