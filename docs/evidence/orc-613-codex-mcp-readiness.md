# ORC-613 · Codex MCP readiness

OP-143's supplied failure reports `Orchester: starting` immediately after a quota resume.
The runner previously requested one inventory after both `thread/start` and `thread/resume`,
then treated every non-connected state as terminal. This establishes the premature failure;
it does not establish why that particular server's initialization took longer.

Codex 0.161.0's [MCP request processor](https://github.com/openai/codex/blob/rust-v0.161.0/codex-rs/app-server/src/request_processors/mcp_processor.rs)
explains the lifecycle: `list_mcp_server_status_response` builds independent discovery unless
both `threadId` and `serverName` are supplied, then attaches `runtimeStatus` from the thread's
`mcp_connection_statuses`. Independent discovery finishing does not mean the thread's
connection finished. With both fields, it instead calls the existing thread's
`mcp_server_status_snapshot`. Thus retrying full inventory could start extra relays and
exhaust Orchester's two-relay listener; readiness rechecks must be targeted.

Required selected servers now wait only on `starting`, using serial targeted reads paced
at 250 ms after a pending answer. A fixed 60-second deadline starts before the first
inventory request and includes unanswered requests. The worker's regular beat checks
the deadline while its stop channels remain live. Missing, failed, disabled, authentication
required, unknown and connected-with-tools-error states still fail explicitly. A starting
runtime waits even when independent discovery has a tools diagnostic. Diagnostics use existing
secret redaction, one-line bounds and project server names. Configuration, permissions,
exemptions, selected servers and relay limits are preserved.

Fresh starts, quota resumes and retries use the same AppServerSession readiness path.
Deterministic fake-App-Server tests cover starting to connected, starting to failed with
redacted diagnostics, missing, permanent starting, unanswered status, cancellation,
automatic/manual retry and quota resume of the same thread. The fake rejects repeated
full inventory and checks that targeted reads name the current thread. No model turn is
sent while a required connection is pending. Non-required servers retain their existing
activity-and-continue behavior.

No live account/quota reproduction or macOS runtime check was performed. No
`cfg(target_os)` code was added or changed. The supplied incident and upstream source
support the readiness race; they do not prove an underlying MCP transport failure.
