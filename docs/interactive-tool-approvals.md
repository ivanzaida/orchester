# Interactive Codex tool approvals

A published Codex pipeline step can opt into **Ask when a tool needs approval**. When Codex requires
permission for an MCP tool call, Orchester keeps the same Codex App Server
session alive and shows the pending agent, server, action and target. **Approve once** answers only
that request. **Deny** ends the step with a visible failure. Other Codex steps continue to use the
noninteractive `codex exec -a never` path. Command, file, sandbox and permission escalation are
not enabled by this setting.

## App Server requests

The verified Codex 0.159 path for an ordinary MCP tool emits `item/started` with an
`mcpToolCall`, followed by `mcpServer/elicitation/request` with an empty confirmation form.
Orchester correlates its thread, turn and server with exactly one active tool call. A real
disposable write tool was independently approved and denied through the desktop and runner;
approval resumed the same session and wrote once, while denial failed the step without writing.

[Codex App Server also documents app tool approvals](https://learn.chatgpt.com/docs/app-server#mcp-tool-call-approvals-apps)
as Accept, Decline or Cancel questions. The local schema names that request
`item/tool/requestUserInput`; the documentation also calls it `tool/requestUserInput`.
Orchester accepts either spelling only for one active MCP call and a blocking, non-secret,
single-choice approval question. Other requests fail the step. Replies never select a session
or permanent grant. No tool-specific implementation depends on Pencil. A real Pencil
`batch_design` call also passed independent approval and denial checks: approval created one
temporary frame and returned its node ID in the same session; denial created nothing. The
temporary frame was removed by that exact ID, preserving the existing design.

## Codex profile

Interactive steps inherit the user's normal Codex account and configured global MCP servers.
Orchester adds the project-selected MCP servers and checks that each selected server is connected
before starting the turn. Other global servers remain available. If Codex is signed out, the step
reports a clear `Not signed in` failure; Orchester does not start a separate login flow.

## Interrupted decisions

An approval is tied to one live process and JSON-RPC request. If the runner or desktop restarts
while a tool waits, Orchester marks the request interrupted and does not automatically resume the
run. The task page shows the interrupted run for a person to review before choosing to continue.
The old decision cannot approve a later tool call.
