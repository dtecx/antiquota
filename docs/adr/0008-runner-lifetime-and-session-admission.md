# ADR 0008: Bound runner lifetime and preserve interrupted session context

Status: Accepted for Slice 1. macOS regression proof passes; Windows and Linux execution await their CI gates.

Stopping only the direct agy process leaves ordinary descendants alive with inherited output pipes. Use pinned `process-wrap` 10.0.1 with its safe Tokio process-group wrapper on Unix and Job Object wrapper on Windows. Its [official API](https://docs.rs/process-wrap/10.0.1/process_wrap/tokio/index.html) provides group/job termination without project-owned unsafe code. A runner-owned guard terminates the group/job on cancellation, leader exit, protocol failure, or future drop. Output tasks have bounded frames, tails, drain time, and completion waits; they are aborted when the runner lifetime ends. Protected real launches retain the denying-hook preflight.

The pure `TurnSink` port carries typed events and conversation context separately. Receiving an init conversation ID immediately appends and snapshots the existing `Meta` event before later tool frames are published. It does not add an event kind, transport envelope, or persisted field. Restart therefore retains the conversation even if no final result arrived.

A session mutex protects a private deletion admission flag before cancellation or storage removal. Commands using a previously cloned session cell must check that flag before admitting work. Deletion waits for its existing worker to stop, preserves usage, removes data, and finally removes the registry entry. Creation reserves an ID briefly, performs storage I/O outside the registry lock, and then publishes its cell.

Server startup is the HTTP-runtime composition boundary: it selects concrete store/account/runner adapters and injects the core ports into app. CLI owns executable startup and discovery; HTTP handlers only translate transport inputs into application commands. App has no concrete adapter dependency. This records the actual small composition boundary rather than adding a separate framework or factory layer.

Proof: `inherited_pipes_close_on_stop_crash_and_protocol_failure`, `stop_and_shutdown_finish_a_turn_with_inherited_descendant_pipes`, `abrupt_engine_exit_after_init_resumes_the_durable_conversation`, `deletion_refuses_admission_through_a_previously_cloned_cell`, and `creation_store_io_does_not_block_existing_session_lookup`. All synthetic agents and data live under `~/Desktop/test`; the abrupt engine-exit test also cleans up its deliberately orphaned synthetic runner.
