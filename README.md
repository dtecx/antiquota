# agyloop

Long-running agent loops on the Antigravity CLI (`agy`), spread over several
Google accounts that all share one stack of skills, hooks, plugins and MCP
servers.

Status: the multi-account layer works. The loop engine (goal mode, builder and
verifier roles, process watcher) and the GUI come next.

## Accounts

```sh
agyloop login b        # sign in a second Google account as slot "b"
agyloop probe          # read /quota per account (no model tokens), wall spent ones
agyloop accounts       # quota, walls and leases per account
agyloop doctor         # prove every account sees the same stack
agyloop run --model gemini-3.8-flash-high -p "..."   # agy on the first account with quota left
```

- `main` is your real HOME and its existing agy sign-in.
- A slot is a fake HOME in `~/.agyloop/slots/<name>`. Its only private files are
  agy's auth state and per-process state (logs, cache, installation id). Every
  other entry links back to the real HOME, so `~/.gemini/config` (skills, hooks,
  plugins, MCP), `GEMINI.md`, claude-mem, gitnexus, git and ssh are shared.
- Sign-in runs under an empty HOME, so it cannot write into the shared stack.
  Whatever else it created is kept in `<slot>/.agyloop-login` for inspection.
- Accounts are used in order (main, then slots by name): one account drains
  before the next takes over. Gemini and Claude/GPT quotas are walled separately.
- One agy process per account at a time (`flock` lease); others wait or move on.
- A quota error is read only from agy's own error channels (stderr on failure,
  JSON error/status fields), never from model text. It walls the account until
  the reset time on the quota panel and exits 75.
- Raw failures are kept in `~/.agyloop/state/failures/` to learn real error shapes.

Run the prober every 10 minutes so walls lift on time:

```sh
*/10 * * * * cd /path/to/agy-loop && python3 -m agyloop probe >> ~/.agyloop/state/probe.log 2>&1
```

## Development

```sh
python3 -m pytest tests -q      # stub agy, no account, no quota
pip install -e .                # or: uv tool install -e .
```
