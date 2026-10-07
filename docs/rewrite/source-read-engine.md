# Engine source-read manifest

Read in full during Phase 1; no source file was replaced by search snippets or a prior-session summary. Numbered, bounded reads covered every line. This manifest is evidence of source coverage, not evidence that rewrite tests pass.

Baseline: commit `17ec296` on `main`. Documentation additions do not change the inspected engine source.

| File | Lines | SHA-256 | Full-read sections |
| --- | ---: | --- | --- |
| `antiquota/__init__.py` | 2 | `abf88a8683054a3e2300f3144ad342762560fdbd058395c13df6b175fb5a153d` | 1–2 |
| `antiquota/__main__.py` | 207 | `4c315e164933dc3c57fa256bbfec8dec723355d3d7cf1034e953ba3d39d87907` | 1–207 |
| `antiquota/accounts.py` | 1155 | `2d01e588983226d531dd229d16cba646c9e2da1e324081f8e9c045e60cd4fb28` | 1–240, 241–480, 481–720, 721–960, 961–1155 |
| `antiquota/convdb.py` | 72 | `4eb64b8ac1f9cc4cd28c8b16f0157858a166c7fe972ffb87afad97302ce27146` | 1–72 |
| `antiquota/engine.py` | 1422 | `dc9d650e484f78055866d012841003e54363284630e5da2cea9365b7b29d0bb3` | 1–250, 251–500, 501–750, 751–1000, 1001–1250, 1251–1422 |
| `antiquota/gitops.py` | 150 | `37272f49c6e9a66b1242f44cf9864297a0cd44cea28c1c3a6672feca432a34fd` | 1–150 |
| `antiquota/hook.py` | 96 | `d89a66038ac7516ebce076d51f6cc8a1523bf5a30364a3405b8349e102f101d3` | 1–96 |
| `antiquota/policy.py` | 72 | `6fd48e7a00b7d454414926b482b98f04e5cfab74277fbdacd180cf79902edf77` | 1–72 |
| `antiquota/server.py` | 700 | `ef4282bfaa050f940384fd4d902bc2e04198b604bf2c990caed44afe6ec84b03` | 1–250, 251–490, 491–700 |
| `antiquota/stack.py` | 483 | `0c5e4aea325772a79a64c3e8abe7262979bf39c68d1641b66d7616f84b8840e5` | 1–240, 241–483 |
| `antiquota/terminal.py` | 120 | `05cfff41e07d10ed37d65c2beea982f98b1b2faf0954dd8ad91d0da45afc57cb` | 1–120 |
| `tests/conftest.py` | 54 | `4f4420966e88c7290baf1c0f61fd4cd54167c438f33d6d99b750791a1b3d624c` | 1–54 |
| `tests/stub_agy.py` | 149 | `2aafb251c3858b4308177fe223c4c30641084b18cabe2614ba7d7ab7c65053f5` | 1–149 |
| `tests/test_accounts.py` | 193 | `df94a6a833a6b329b7d841ef578d3150ee67cfeb7f7358a85e7d575c8d567d4d` | 1–193 |
| `tests/test_convdb.py` | 32 | `613cefdfe1c3c6641fe9241b475be80c178ba595151155d9547b9ff2dc1ffedc` | 1–32 |
| `tests/test_engine.py` | 454 | `7dd849dae9ed7f2a1648f6131ade4246d7ff368ff34ab454611b06626c202482` | 1–230, 231–454 |
| `tests/test_gitops.py` | 90 | `375e5c1dd8f0ef40fefa6e42b5578e66b52ed5e6a2b4fb5cfbd269812ff64344` | 1–90 |
| `tests/test_policy.py` | 42 | `17dd24134066985046f7e58c0ed4fb4afee8f61c5ce8e4b862017fcf3cb37e68` | 1–42 |
| `tests/test_server.py` | 203 | `51c4a4191fc3eab1a19291f9462bbf42f8e506ac6528834eb9ea4b59c27fc57a` | 1–203 |
| `tests/test_stack.py` | 82 | `a96343a6e49dbeba3e7d1345aff176c2df717549bb5a55a360785413bd190117` | 1–82 |
| `pyproject.toml` | 15 | `31b05e416ecfa9b4d634148f52e6e80d6948828dca13cecfe05e48f7b9ee9c1c` | 1–15 |
| `tests/quota_sample.json` | 64 | `5dfb49cfda9ab3ad9843b003d9bd5a0d3cde31f3357b7373276624850477f727` | 1–64 |
| `tests/quota_sample_spent.json` | 62 | `dfaf2158013fa17b5f2a142b1c9dde6be7d27799a97ef9a264ec67bd32260f24` | 1–62 |

## Coverage details

- `accounts.py`: storage roots and old-folder compatibility; stack cloning and path rewriting; slot farms and sign-in; verification and removal; walls/leases; quota routing and history; direct CLI execution.
- `engine.py`: constants and utility parsers; keep-awake; session/event persistence; lifecycle/git/fork/search/dictation/PTY; attachments/queue/goals; turn retry/handoff and stream reduction; thinking/checkpoints/rewind; jobs/schedules; approvals/steering; shell/notes/files; verifier and deterministic state.
- `server.py`: static assets/token generation; application caches/sign-in/permissions/folder completion; HTTP dispatch including the six routes outside the match; every match case; terminal/global SSE; daemon locking/discovery/start/shutdown.
- `stack.py`: source kinds/digests/overview; copy/delete/edit/toggle; defaults and numeric ranges; permission shell parsing/RISKY flags; hook installation and quoting.
- `policy.py`, `hook.py`, `gitops.py`, `terminal.py`, `convdb.py`, `__main__.py`, `__init__.py`: complete numbered files, including all tools, commands, flags, fallbacks and platform branches.
- All Python tests, `conftest.py`, `stub_agy.py`, both real quota samples and `pyproject.toml`: complete reads, with assertions traced to relevant inventory items.

## Inventory checks

The associated [FEATURES-engine.md](FEATURES-engine.md) has 397 distinct acceptance items (`F-001` through `F-397`), including 75 HTTP method/path records. The 75 routes comprise 69 match cases and six special dispatch routes. Each item links to pending rewrite proof and names the related legacy test when one exists.

Legacy coverage is incomplete: several listed edge cases currently have no targeted test. A related broad legacy test does not imply it exercises every subcase in a feature row. The rewrite must produce explicit passing proof for every feature id.

The canonical [FEATURES.md](FEATURES.md) contains these full engine rows without renumbering. Their rewrite evidence links to [TEST_REPORT.md](TEST_REPORT.md); all remain Pending at consolidation.
