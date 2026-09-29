# Interactive Codex tool approvals

A published Codex pipeline step can opt into **Ask for each tool action**. When a selected MCP
server asks the host for permission to run one tool call, Orchester keeps the same Codex App Server
session alive and shows the pending agent, server, action and target. **Approve once** answers only
that request. **Deny** ends the step with a visible failure. Other Codex steps continue to use the
noninteractive `codex exec -a never` path. Command, file, sandbox and permission escalation are
not enabled by this setting.

## App Server requests

[Codex App Server documents MCP tool-call approval requests](https://learn.chatgpt.com/docs/app-server#mcp-tool-call-approvals-apps)
as a short Accept, Decline or Cancel question before a side-effecting app tool runs. The local
Codex 0.159 schema names that request `item/tool/requestUserInput`; the documentation also calls
it `tool/requestUserInput`. Orchester accepts either spelling only when it identifies one active
MCP call and has a blocking, non-secret, single-choice approval question. It also accepts a
`mcpServer/elicitation/request` only for an empty confirmation form correlated to exactly one
active MCP call. Other requests fail the step. Which request Pencil `batch_design` sends, and
whether its write completes after approval, still need a live test with a disposable active Pencil
document.

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
