# ADR 0034: native shutdown and microphone signing

The desktop shell exits an app-owned engine only after its shutdown request succeeds. A refused request leaves the shell open, explains the failure and resets the quit latch so the owner can retry. Choosing background work still leaves the engine running; quitting the shell still preserves an externally started engine.

A turn cancelled by Stop finishes before newly queued work begins. Stop clears older queued requests and pauses the goal in memory before fallible persistence; a failed approval or metadata write cannot revive older requests. Messages or goal resumes accepted afterward receive a fresh cancellation token when the old worker drains them. Precomputed follow-ups from the cancelled turn remain suppressed. Engine shutdown prevents this renewal and retains its existing queue behavior. A virtual-time regression covers immediate Resume, immediate Send, discarded older work, both Stop write failures, durable recovery and shutdown without timing sleeps.

The engine-page capability names both localhost and 127.0.0.1. Navigation still accepts only the discovered engine's exact origin; this does not permit navigation to another local server.

macOS microphone access needs both the existing usage description and the hardened-runtime audio-input entitlement. Tauri receives an Entitlements.plist containing only that entitlement. Developer signing uses this configuration. The local ad-hoc packaging step explicitly supplies the entitlement and hardened runtime: an unsigned Tauri bundle has no microphone metadata to preserve. Final signatures are inspected after packaging.

Native UI evidence uses a distinct test identity and the existing debug-only background mode. A hardened signature on that test shell supplies the required microphone signing metadata without claiming successful recording or a production-identity launch. OS storage paths and LaunchAgents do not reliably follow HOME; the report records their actual destinations and cleanup. Real login startup, release notarization and unexercised native actions remain separate acceptance checks.
