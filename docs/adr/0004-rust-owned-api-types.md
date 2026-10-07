# ADR 0004: Generate API types from Rust

Status: Accepted. Generated files and stale-check proof are pending.

The 75 captured routes and fixture comparisons define HTTP compatibility. Server owns transport request/response structs; core owns reusable domain/event types. Both derive `serde` and `ts-rs::TS`. TypeScript never declares parallel API interfaces.

Use ts-rs 12.0.1 and explicit `Config` with `TS::export_all(&Config)`. A server exporter generates root types and dependencies into `ui/src/gen/`, plus a deterministic Rust-owned route catalog. Generation into a temporary directory enables a stale check that compares names and bytes without modifying tracked output. [Export API](https://docs.rs/ts-rs/12.0.1/ts_rs/trait.TS.html#method.export_all), [Config API](https://docs.rs/ts-rs/12.0.1/ts_rs/struct.Config.html).

Enable `serde-compat` and `serde-json-impl`. The downloaded 12.0.1 source implements JSON values as a recursive `JsonValue` union, without `any`; unknown stored fields can therefore remain lossless JSON. Set `TS_RS_LARGE_INT=number` to match existing JSON numerics and reject out-of-range public counters rather than lose precision. [Supported Serde attributes and integer configuration](https://docs.rs/ts-rs/12.0.1/ts_rs/).

One typed API client owns JSON/raw transport and errors. One event store owns EventSource. A narrow parser boundary validates unknown wire data before reducers consume generated types. Generated code is exempt from manual style rewriting; its source remains Rust.

Proof: generated source contains no `any`, union payloads match fixtures, all root types export, stale checks reject changed/missing/extra files, and strict client/reducer tests pass.
