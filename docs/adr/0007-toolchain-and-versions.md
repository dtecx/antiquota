# ADR 0007: Pin verified compatible toolchains

Status: Accepted for initial implementation. Application dependency lockfiles and complete checks remain pending.

Registry metadata was checked on 2026-10-07. Use committed Cargo and pnpm lockfiles and frozen installs. Start with Rust 1.99.0, Node 26.10.0, and pnpm 11.19.0; these are installed locally. Homebrew-installed `just` 1.58.0 and cargo-deny 0.20.2 were verified with their version commands. Local install logs stay ignored. [Just](https://just.systems/), [cargo-deny](https://github.com/EmbarkStudios/cargo-deny).

Initial Rust versions are axum 0.8.9, Tokio 1.53.2, serde 1.0.229, serde_json 1.0.151, thiserror 2.0.21, anyhow 1.0.104 for executables, tracing 0.1.44, tracing-subscriber 0.3.23, ts-rs 12.0.1, clap 4.6.7, and proptest 1.11.0. Add portable-pty 0.9.0 in the terminal slice. Add libraries only when their owning slice needs them. A safe keepawake 0.6.1 handle is a candidate for platform sleep inhibition and must pass target tests before adoption. [Cargo registry](https://crates.io/), [axum server API](https://docs.rs/axum/0.8.9/axum/fn.serve.html), [portable-pty](https://docs.rs/portable-pty/0.9.0/portable_pty/).

Initial UI versions are Svelte 5.57.2, TypeScript 6.0.3, Vite 8.3.3, vite-plugin-svelte 7.3.1, svelte-check 4.7.6, Vitest 5.0.3, Playwright 1.63.0, ESLint 10.12.0, typescript-eslint 8.71.1, eslint-plugin-svelte 3.23.0, Prettier 3.9.9, and prettier-plugin-svelte 4.1.1. Their engines support Node 26.10.0 and peer ranges support this combination. Add xterm 6.0.0/fit 0.11.0 and Chart.js 4.5.1 when their views arrive. [npm registry](https://registry.npmjs.org/).

TypeScript 7.0.2 was current but is incompatible with svelte-check's `^5 || ^6` range and typescript-eslint's `<6.1` range. Choose 6.0.3 instead of overriding peer constraints. A frozen install and strict checks must prove the selected dependency graph; metadata checks alone do not.

One justfile runs both toolchains. `dev` uses a disposable Desktop/test root, fake-agy, disabled conversation-link creation, and a separate port by default. The real Python engine remains available on 8765; default development never touches its data or costs model tokens. `check` denies Rust warnings and runs cargo-deny, formatting, strict UI validation, lint/format checks, and stale type checks. `test` runs offline Rust/Vitest/Playwright suites; native and paid live scenarios have separate evidence. Bound local compile jobs. CI covers all three requested OSes, but remote runs need the user's future push authorization. No unrun matrix entry is green.

Proof: frozen dependency resolution, all local checks, generated-type stability, each OS test/build job, and native bundling. Recheck and record any version adjustment rather than silently changing the baseline.
