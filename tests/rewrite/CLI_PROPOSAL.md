# Shared-session CLI proposal

An additive `chat` command awaits the user's public CLI decision. It is not wired into the binary or included in compiled acceptance tests. Existing `run` retains its legacy arbitrary argument, output, native terminal, account routing and exit-code behavior.

If the user accepts `chat`, its acceptance cases must prove that a discovered engine remains unchanged, the request becomes a durable shared session visible through HTTP, the current turn streams in the CLI, and usage survives restart. A no-account request must fail promptly and preserve its accepted user message without launching agy. These proposed cases do not establish legacy `run` parity.

The owner's 2026-10-11 Claude-style CLI request is delivered additively as `antiquota -i` (ADR 0037), including exact `--resume`, shared streaming sessions and approvals. The named `chat` command remains absent; bare account listing and raw `run` remain unchanged. Compiled real-PTY/HTTP acceptance lives in `interactive.rs`.
