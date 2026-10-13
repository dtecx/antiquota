# ADR 0040: Busy account quota refresh

The owner noticed on 2026-10-13 that account cards read "refreshed 1h" and "refreshed 2h" although quotas refresh every 10 minutes. Automatic refreshes took the exclusive account lease and skipped any account with a running call. With goal agents holding one account for hours, its reading never refreshed.

Model calls already share an account HOME (ADR 0025), so a quota read is no riskier than another shared call; the token-refresh race (R-011) remains the same accepted risk. The periodic refresh now tries the exclusive lease first and, when the account is busy, reads it under a shared lease. A walled account still waits for an exclusive probe, so a shared reading cannot clear a failure that a running call just published (ADR 0033). Post-turn probes, manual Check and removal keep the exclusive lease.

The polling interval itself is unchanged: it follows the quota poll setting, which an existing contract pins.
