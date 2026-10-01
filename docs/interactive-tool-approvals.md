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

Every App Server turn explicitly selects the local environment with its current worktree as the
runtime workspace root, including resumed sessions. An empty environment list disables local tools
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

## Claude Code

A Claude Code profile, pipeline step or run can use the same two choices. Claude Code has no App
Server; Orchester uses its print-mode permission prompt instead (`--permission-prompt-tool`, hidden
from `claude --help` in 2.1.89 but present in the binary as "MCP tool to use for permission
prompts (only works with --print)"). The invocation gets a second `--mcp-config` file that starts
`orchester-runner --claude-permission-prompt`, an MCP server named `Orchester` whose one tool,
`approve`, is the prompt (`mcp__Orchester__approve`). Claude Code calls it only when its own
permission check would ask a person, and removes it from the tools the model may call. The relay
hands each call to the runner over a loopback connection opened with a random per-invocation
token; the configuration file holding the token is private and removed when the CLI ends.

What reaches a person is decided by `ClaudeApprovalScope` in core: a `Bash` command when the step
asks before commands, and a tool of one of the invocation's own MCP servers when it asks before
MCP actions. For commands, a writing step gets `--settings {"permissions":{"ask":["Bash"]}}`: a
rule asking for the whole `Bash` tool is checked before the tool's own check (both 2.1.89 and
2.1.284), so every command reaches the prompt, including the `mkdir`, `touch`, `rm`, `rmdir`, `mv`,
`cp` and `sed` that `acceptEdits` would otherwise run, and the commands Claude Code classifies as
read-only. That is stricter than Codex, whose known-safe read-only commands run without asking. A
read-only step gets no `Bash` at all: Claude Code would run an approved command outside any
sandbox, where it could write. For MCP actions the `mcp__<server>` allow rules are left out, and a
read-only step runs in permission mode `default` instead of `dontAsk`, which would deny before
asking. Everything else Claude Code asks about (edits or reads
outside the working folders, web fetches, the approval tool itself) is denied at once, as print mode
denies it without the prompt. Every MCP tool call of a server asks; Claude Code does not pass the
server's own approval hints to the prompt.

**Approve once** answers `{"behavior":"allow","updatedInput":{}}`: the call runs with its own input
(2.1.89 keeps the input when `updatedInput` is empty), no `updatedPermissions` rule is returned, and
the input is never echoed back. **Deny** answers `{"behavior":"deny"}` and fails the step, as for
Codex. Decisions use the same `tool_approvals` rows, ids (`<invocation>:<tool use id>`), claim and
delivery states and the same "Needs you" modal. A tool use id is asked about once per invocation; a
repeated call is denied without a prompt. A decision that does not match the waiting call, arrives
after the agent ended or names an earlier invocation is refused (`ToolApprovalNotAwaited`).

While a call waits, the invocation's active time stops (`waiting_since`). A cancel ends the agent
and marks the request interrupted. A pause or a graceful shutdown checkpoints the session at the
waiting call, as for Codex: the call is not granted, the request ends interrupted, the work is saved
and the step continues when the run does. An approved call whose
result has not arrived when the agent ends is recorded as an interruption, so the run needs a
person's review before it continues. Project MCP secret values are redacted from the request's
action, target and scope and from the invocation's activity. Claude Code runs an approved command
without a sandbox, as it runs every command; the person sees the whole command first.

With *Allowed commands* instead, a writing step denies the commands `acceptEdits` would run in any
form without a rule (`mkdir`, `touch`, `rm`, `rmdir`, `mv`, `cp`, `sed`) with `Bash(<command>:*)`
deny rules. A prefix deny rule wins over every allow rule in both 2.1.89 and 2.1.284, so it would
also deny a listed `sed -n` or `rm -f build/x`. A writing step whose allowed commands contain one of
these words therefore does not run in `acceptEdits`: it runs in `dontAsk` (`default` when it asks
before MCP actions) and allows its edits with an `Edit(//<folder>/**)` rule for the worktree and
each extra writable folder, which both versions accept like `acceptEdits` accepts an edit in a
working directory, after the same deny rules and safety checks. Only the listed prefixes of those
commands then run: `sed -n 1p README.md` runs, `sed -i …` is denied; `rm -f build/x` runs,
`rm -rf src` is denied. A prefix still covers what follows it, as every allowed command does
(`sed -n -i …` starts with `sed -n`). The enforcement panel names this mechanism *allowed tools ·
edit rules per folder*. Commands Claude Code classifies as read-only still run without being
listed, once the list has an entry (with none, the step gets no `Bash` at all). That is the
user's decision (`docs/plan.md`, C4) and the same as Codex, whose known-safe read-only commands run
without asking: they change no file. The option's text says so (*These commands run, as do
known-safe read-only commands such as ls, cat or git log*), and the enforcement panel, the run
permissions popover and the pipeline step's commands add "Claude Code also runs known-safe
read-only commands, such as ls, cat or git log, without listing them."

A managed policy with `allowManagedPermissionRulesOnly` makes Claude Code ignore every rule Orchester passes (`--allowedTools`, `--disallowedTools`, the `--settings` ask rule), so none of the above is enforced on such a machine.
