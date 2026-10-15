# ADR 0043: Offline sign-in failures and sleeps that look again

On the evening of 2026-10-15 the network failed for about forty minutes. agy waited 30 seconds for each token refresh and then reported `error getting token source: You are not logged into Antigravity.` There was no network word in that message, so antiquota read it as a real sign-out. Within twenty minutes it parked three healthy accounts for 30 days, one after another. The two remaining accounts had spent their weekly quota, so all six goals went to sleep until the earliest weekly reset, five days away. Later probes found the three accounts healthy and cleared their walls, but the sleeping goals did not notice.

Two changes follow.

- A sign-in failure is trusted only while Google's sign-in host answers. When a turn or a quota probe reports a sign-in failure, antiquota first opens a connection to `oauth2.googleapis.com:443` with a five-second limit. If it fails, the turn counts as offline (the reconnect loop of ADR 0041 waits and retries it) and the probe is unreadable. A real sign-out, with the network up, still parks the account. `ANTIQUOTA_SIGNIN_PROBE` overrides the host for tests: `online` skips the check, and a `host:port` is tried instead.
- A goal that sleeps on spent quota looks at the accounts again at least every two minutes instead of once at the reset. It posts one notice per sleep and stays `sleeping` between looks, so a wall that lifts early (a probe, a new sign-in) wakes it at once. It now wakes at the reset itself rather than 30 seconds after it.
