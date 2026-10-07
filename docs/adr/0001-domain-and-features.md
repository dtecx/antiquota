# ADR 0001: Keep the domain pure and feature hooks small

Status: Accepted for the rewrite; implementation proof pending.

Core owns types, state transitions, routing, permission decisions, and the nine requested port traits. It has no Tokio, filesystem access, process spawning, or transport dependency. Features depend on core and implement `before_turn`, `on_event`, and `after_turn`; they return commands for application services to execute.

Each goal/verifier, plan, loop/schedule, jobs, handoff, steering, fork, and keep-awake responsibility has a small module. Application wiring lists features explicitly. There is no runtime plugin registry, container, generic hub, or feature logic inside adapters. A new feature needs its module and a composition entry rather than edits throughout turn execution.

App owns waiting, process cancellation, event publication, and per-session command serialization. A short registry lock does not cover I/O or waiting. This keeps policy/property tests deterministic and lets stop/approval commands interrupt a worker. Fake ports and a fake clock exercise the same application services as production.

Libraries use `thiserror`; executables may use `anyhow`. Each source module states its ownership and stays below about 400 lines. Project-owned unsafe code is confined to the PTY adapter; safe platform libraries handle other native operations.

Proof: dependency checks for core purity, hook ordering tests, cancellation tests, feature-specific integration tests, and module-size/unsafe review.
