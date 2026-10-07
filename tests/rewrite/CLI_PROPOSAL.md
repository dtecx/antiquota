# Shared-session CLI proposal

An additive `chat` command awaits the user's public CLI decision. It is not wired into the binary or included in compiled acceptance tests. Existing `run` retains its legacy arbitrary argument, output, native terminal, account routing and exit-code behavior.

If the user accepts `chat`, its acceptance cases must prove that a discovered engine remains unchanged, the request becomes a durable shared session visible through HTTP, the current turn streams in the CLI, and usage survives restart. A no-account request must fail promptly and preserve its accepted user message without launching agy. These proposed cases do not establish legacy `run` parity.
