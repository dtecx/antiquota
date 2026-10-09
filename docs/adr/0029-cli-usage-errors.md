# ADR 0029: CLI usage errors

Status: Accepted for Slice 17a.

The legacy CLI printed its usage text to stderr and exited 2 for an unknown command or a malformed one. clap exits 2 for the same cases, but it leaves the usage line out of some messages, such as an invalid value. `main` now parses with `try_parse` and prints the usage line first when clap's message lacks it, then lets clap print its error and exit.

clap is stricter than the legacy parser in one place: `stack` with a word other than `reseed` is refused, where the legacy CLI ignored the word. Refusing is safer, since a mistyped `reseed` would otherwise do nothing silently.
