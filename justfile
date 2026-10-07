# Owns the Rust and UI toolchain entry points. All development agent calls are synthetic.
set shell := ["sh", "-eu", "-c"]

default:
    @just --list

dev: build-engine
    pnpm --dir ui install --frozen-lockfile
    pnpm --dir ui exec node scripts/dev.ts

build-engine:
    cargo build --workspace --jobs 2

build-ui:
    pnpm --dir ui install --frozen-lockfile
    pnpm --dir ui build

build: build-engine build-ui
    @echo 'Rust engine and browser UI built. Desktop shell packaging is Pending until Slice 10.'

build-shell:
    @echo 'Desktop shell packaging is Pending until Slice 10.' >&2
    @exit 1

gen-types:
    cargo run --jobs 2 -p antiquota-server --example export_types

check-types:
    cargo run --jobs 2 -p antiquota-server --example export_types -- --check

check: check-types
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --jobs 2 -- -D warnings
    cargo deny --config .cargo/deny.toml check
    pnpm --dir ui check
    pnpm --dir ui lint
    pnpm --dir ui format

test-unit:
    cargo build --jobs 2 -p antiquota-cli -p fake-agy
    cargo test --workspace --jobs 2
    pnpm --dir ui test

test-e2e: build-engine build-ui
    pnpm --dir ui test:e2e

test: test-unit test-e2e
