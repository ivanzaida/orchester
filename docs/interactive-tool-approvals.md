# Interactive Codex approvals

A Codex profile or a current run can enable **Ask before MCP actions that require approval**.
A published pipeline step can require it too. Separately, **Ask before running commands** enables
decisions for Codex commands that require approval; ordinary sandboxed commands can still run
without a request. These choices determine the transport from the effective invocation permissions,
including current-run overrides, rather than only the saved pipeline checkbox.

Orchester uses Codex App Server for either choice and displays each pending request. **Approve once**
answers only that action within the step's existing sandbox grants. Sandbox exceptions are disabled;
approval cannot add writable folders or network access. **Deny** prevents the action and fails
the step visibly. File-change approvals, additional permission grants and unsupported approval
request forms are rejected explicitly. Codex invocations with neither choice keep the noninteractive
`codex exec -a never` transport.

Changing permissions from a waiting task's popover applies to the next invocation. When continuing
the task conversation, an existing `exec` session can resume through App Server with its original
session ID and worktree. Completed steps, uncommitted work, run limits and the saved pipeline remain
intact. **Apply now** uses the existing checkpoint/restart mechanism. Approval requests and waiting
time are persisted; waiting for a decision does not consume active agent time.

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

Both read-only and workspace-write Windows invocations explicitly select Codex's native elevated
sandbox, including resumed `exec` invocations and App Server. Selecting that sandbox does not change
read-only access into write access. Windows network restrictions retain their existing visible
capability limitation.

Every App Server turn explicitly selects the local environment with exactly the current worktree
and, for write access, its authorized additional writable folders as runtime workspace roots,
including resumed sessions. Read-only invocations explicitly clear configured writable roots.
An empty environment list disables local tools
in Codex 0.159.2 and is never sent. Startup validates the selected environment and combines its
implicit workspace root with the extra sandbox writable roots when checking the exact grants.
Project MCP secret values are redacted from commands, output, messages, errors and approval metadata
before they reach activity tracking or storage, including their JSON-escaped form.

## Build directories

An absolute project `CARGO_TARGET_DIR` is a build root. Agents and command steps in a worktree receive
`<root>/worktrees/<worktree-directory>`, and a command tool's own absolute override is scoped the same
way. Sequential steps and resumed invocations in one worktree reuse that directory; parallel
worktrees use distinct directories. Relative values already resolve inside a worktree and are kept.
The configured project environment is not rewritten. New worktrees start cold, so the host preflight
requires the cold-target free-space reserve even when the build root contains other cached artifacts.

## Interrupted decisions

An approval is tied to one live process and JSON-RPC request. If the runner or desktop restarts
while a tool waits, or after approval was delivered before the turn completed, Orchester records the
interruption and does not automatically resume the
run. The task page shows the interrupted run for a person to review before choosing to continue.
The old decision cannot approve a later tool call.
