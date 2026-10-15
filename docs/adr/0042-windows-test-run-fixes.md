# ADR 0042: Fixes from the first Windows test run

The Rust suite had never run on Windows. On 2026-10-15 it ran on a Windows 11 PC (build 26100, Rust 1.99, MSVC) and 23 of 498 tests failed. Most failures were tests that assume a POSIX shell. Four were real Windows bugs.

- Sign-in. The authorization code was written to agy's console followed by `\n`. ConPTY treats Enter as a carriage return, so agy never received the line and sign-in hung until it timed out. On Windows the code now ends with `\r`. A cancelled sign-in also retries removing its temporary folder for up to two seconds, because Windows can hold a just-ended process's folder for a moment.
- Paths. `std::fs::canonicalize` returns `\\?\C:\…` on Windows. The CLI's working folder for `antiquota -i` and `antiquota goal` and the stack path shown in Settings now use the existing `canonical` helper, which drops that prefix. The stack root used for containment checks stays as it was, so its comparisons keep matching.
- Rewind. Checkpoint keys for a new nested file were built two ways: `project\new/a.txt` before the folder existed and `project\new\a.txt` after. The keys did not match, so the edit could not be rewound. The checkpoint path is now rebuilt from its components, which gives one key on every platform.
- Terminal panel. ConPTY keeps its output open after the shell exits, so typing `exit` never ended the panel. On Windows a watcher now checks the shell process; when it ends, the pseudo-console closes and subscribers get the exit.

Tests that already meant to run on Windows were corrected: the goal check uses `if exist` under cmd, two `set /p` commands reset cmd's exit code with `(call )`, and three path expectations join components instead of writing `/`. Seven tests are ignored on Windows, each with its reason: three replay captured fixtures that run POSIX commands, two expect the exact line echo of a Unix terminal (ConPTY redraws lines with erase codes), one checks interplay with the legacy Python app's `fcntl` locks, and one starts a background engine inside the test console's kill-on-close job, which also ends the engine.

A background engine started by `antiquota -i` inside a console that uses a kill-on-close job still ends with that console. Normal terminals are not known to do this; it is not handled.
