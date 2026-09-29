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
active MCP call. Other requests fail the step. The isolated Codex home is not yet signed in, so
which request Pencil `batch_design` sends, and whether its write completes after approval, still
need a live test with a disposable active Pencil document.

## One-time Codex login

Codex App Server cannot ignore the normal user's MCP configuration as `codex exec` does. Orchester
therefore gives each project a separate Codex home. It begins empty and signed out. The first
interactive step stops with a `Not signed in` event containing the exact login command for that
project. Run that command in a terminal and retry the step. Orchester never copies an existing
Codex token or signs in on the user's behalf.

On Windows, the command shown by the step has this form in PowerShell:

```powershell
$env:CODEX_HOME = 'C:\Users\<user>\AppData\Local\OrchesterApprovalHomes\<project-id>'; codex login
```

On macOS, it has this form:

```sh
CODEX_HOME="$HOME/Library/Application Support/OrchesterApprovalHomes/<project-id>" codex login
```

The path in the step's event is authoritative. Orchester verifies that the App Server's active MCP
inventory contains exactly the servers selected by the agent profile before starting the turn. If
the isolated home has extra MCP servers, the step fails with a configuration error; remove those
servers from that home before retrying.

## Interrupted decisions

An approval is tied to one live process and JSON-RPC request. If the runner or desktop restarts
while a tool waits, Orchester marks the request interrupted and does not automatically resume the
run. The task page shows the interrupted run for a person to review before choosing to continue.
The old decision cannot approve a later tool call.
