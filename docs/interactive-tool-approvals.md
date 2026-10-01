# Interactive approvals

What an agent's permissions do not allow by themselves asks a person in "Needs you" instead of being
refused (the user's decision 2026-10-01, "List + ask"): with *Allowed commands*, Claude Code runs the
listed commands and its known-safe read-only ones, and any other command asks you first; with *Any
command in the worktree*, Codex runs commands in its sandbox, and a command that needs more than the
sandbox asks you first. A Codex profile or a current run can also enable **Ask before MCP actions
that require approval**; a published pipeline step can require it too. **Ask before running
commands** sends every Claude Code command to a person, and every Codex command an execpolicy rule
asks about. These choices come from the effective invocation permissions, including current-run
overrides, rather than only the saved pipeline checkbox.

Every Codex step runs as Codex App Server, and Orchester displays each pending request. The thread's
granular approval policy turns on `sandbox_approval` (always), `rules` (with *Ask before running
commands*) and `mcp_elicitations` (with *Ask before MCP actions*); `request_permissions` and
`skill_approval` stay off. With `sandbox_approval` Codex asks before it runs a command outside its
sandbox: when the model requests it (`sandbox_permissions: "require_escalated"`, with its
justification as the question), and when a command failed in the sandbox for want of network or
write access. Codex 0.159.2's instructions to the model name both (network while it is off, a write
outside the writable roots, GUI apps, destructive commands it was not asked for). **Approve once**
answers `accept` for exactly that request: Codex runs that one command outside the sandbox; no
session grant, `acceptForSession`, execpolicy amendment or network policy amendment is ever chosen.
**Deny** prevents the command and fails the step visibly. A request with an additional permission
overlay (`with_additional_permissions`, off unless `features.exec_permission_approvals` is enabled)
and unsupported request forms fail the step. Two requests are declined at once without a person: a
file change outside the writable roots (`item/fileChange/requestApproval`: edits keep their rules,
only commands are asked about) and a host approval of Codex's managed network proxy
(`networkApprovalContext`, whose grant may outlast the command). A read-only step keeps the
`read-only` sandbox, and its escalations ask as well; the approval says that an approved command
runs outside the sandbox, where it can change files.

Changing permissions from a waiting task's popover applies to the next invocation. When continuing
the task conversation, a session that ran through `codex exec` before App Server became the only
transport resumes through App Server with its original session ID and worktree. Completed steps, uncommitted work, run limits and the saved pipeline remain
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

App Server has no `--ignore-user-config` or `--ignore-rules`, so it loads the user's normal Codex
account and configuration. Orchester keeps what of it would reach the agent out (checked against
0.159.2 with `config/read` and `mcpServerStatus/list`, without a model turn), except the user's own
execpolicy rules, which are the user's choice (user decision 2026-10-01):

| From the user's setup | Neutralised by |
| --- | --- |
| `notify` program | `-c notify=[]` (an array replaces the user's) |
| plugins, their skills and MCP servers (`code-review`, `cua_repl`, …) | `-c features.plugins=false` |
| apps (`codex_app`) | `-c features.apps=false` |
| permission overlays that can stay granted (`with_additional_permissions`, `request_permissions`) | `-c features.exec_permission_approvals=false`, `-c features.request_permissions_tool=false` |
| the user's MCP servers | each turned off for the thread by name (`config` of `thread/start`/`thread/resume`: `mcp_servers.<name>.enabled=false`), the names read with `config/read` first; a table merges with the user's, so `mcp_servers={}` would not remove them. The thread's inventory must then show every server the step does not choose `disabled`, or the step fails. A server of the user's own configuration named like one the step chooses would merge its `command`, `args` or `env` into it (Codex merges the tables of its layers), so the step stops and says to rename one. |
| hooks | cannot be turned off: a configuration that defines any refuses the step |
| the user's execpolicy `.rules` (`$CODEX_HOME/rules`) | **kept**, as the user's choice (0.159.2 has no App Server setting for them anyway). A command an `allow` rule matches runs outside the sandbox without asking. The enforcement panel says *Your own Codex rules still apply · allow rules run unsandboxed, unasked*, and the run permissions popover says it under Commands. |

The repository's own `.codex` folder (`config.toml`, `hooks.json`, `rules/`, its MCP servers) is never
loaded, whatever the user's configuration trusts. Every start marks the worktree and its repository
untrusted with `-c projects={"<worktree>"={trust_level="untrusted"},"<repository>"={…}}`. Codex
0.159.2 looks a project's trust up by the folder it works in, then by the repository a worktree belongs
to; a parent folder the user trusts does not make a repository trusted. A key matches on Windows only
with `\` separators and without the `\\?\` prefix (the drive letter's case does not matter), so the
keys are written that way; an inline table merges with the user's `projects`, while a dotted key would
split at the dot of `.orchester`. Checked on 2026-10-01 without a model turn, in a scratch repository
whose worktree has an allow rule, a server, a hook and a skill under `.codex`, under scratch
`CODEX_HOME`s that trust the repository (either drive-letter case), the worktree, a parent folder or
the drive: without the override the trusted cases loaded the server, the hook and the rule (the
execpolicy log line `loaded 1 .rules files in …\.codex\rules`); with it `config/read` lists the folder
as a project layer disabled because the folder *is marked as untrusted*, and nothing of it loads.
Before each thread the runner reads `config/read` with its layers: a project layer Codex loaded all the
same refuses the step, and a disabled one adds *This branch has .codex/ settings; Orchester ignores
them* to the activity without stopping the step. Codex still loads the skills of `.codex/skills` in
an untrusted repository; they follow *Repository agent files* like `.agents/skills` (disabled by path
while it is off).

The user's model, reasoning effort and service tier defaults still apply where the profile sets none.
Orchester adds the project-selected MCP servers. A step that asks before MCP actions checks that each
selected server is connected before starting the turn; any other step works without a selected server
Codex did not connect, and its activity names it. If Codex is signed out,
the step reports a clear `Not signed in` failure; Orchester does not start a separate login flow.
Codex's standard error is kept in the step's activity as raw lines. A turn's usage adds the `last`
usage of each `thread/tokenUsage/updated` whose thread total moved, so a model response is counted
once even when Codex repeats the notification; Codex reports usage per model response, so a run or
step budget applies when the turn ends.

Both read-only and workspace-write Windows invocations explicitly select Codex's native elevated
sandbox. Selecting that sandbox does not change
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

A Claude Code profile, pipeline step or run can use the same two choices, and a step with allowed
commands asks about the others. Claude Code has no App Server; Orchester uses its print-mode permission prompt instead (`--permission-prompt-tool`, hidden
from `claude --help` in 2.1.89 but present in the binary as "MCP tool to use for permission
prompts (only works with --print)"). The invocation gets a second `--mcp-config` file that starts
`orchester-runner --claude-permission-prompt`, an MCP server named `Orchester` whose one tool,
`approve`, is the prompt (`mcp__Orchester__approve`). Claude Code calls it only when its own
permission check would ask a person, and removes it from the tools the model may call. The relay
hands each call to the runner over a loopback connection opened with a random per-invocation
token; the configuration file holding the token is private and removed when the CLI ends.

What reaches a person is decided by `ClaudeApprovalScope` in core: a `Bash` command when the step
asks before commands or has allowed commands (one outside the list), and a tool of one of the
invocation's own MCP servers when it asks before MCP actions. Every Claude Code step therefore has
the prompt ("agents ask"), and none runs in `dontAsk`. For commands, a step that asks before them, or
lists none, gets `--settings {"permissions":{"ask":["Bash"]}}`: a
rule asking for the whole `Bash` tool is checked before the tool's own check (both 2.1.89 and
2.1.284), so every command reaches the prompt, including the `mkdir`, `touch`, `rm`, `rmdir`, `mv`,
`cp` and `sed` that `acceptEdits` would otherwise run, and the commands Claude Code classifies as
read-only. That is stricter than Codex, whose known-safe read-only commands run without asking. A
read-only step asks too, in permission mode `default` (`dontAsk` would deny before asking), with the
edit tools removed; its approval says that Claude Code runs an approved command outside any sandbox,
where it can change files. For MCP actions the `mcp__<server>` allow rules are left out. Everything else Claude Code asks about (edits or reads
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

With *Allowed commands* ("List + ask"), the listed prefixes become `Bash(<prefix>:*)` allow rules,
and the step runs in permission mode `default` with the permission prompt, read-only and writing
alike. Commands Claude Code classifies as read-only run without being listed, once the list has an
entry (with none, every command asks, as when asking before commands; the panel reads *no listed
commands · all ask*). That is the user's decision
(`docs/plan.md`, C4) and the same as Codex, whose known-safe read-only commands run without
asking: they change no file. Any other command asks a person. In `default` mode every check that
would ask hands the call to the prompt: an unlisted command, a listed one with a `$VAR` expansion,
a compound command (`;`, `&&`, `|`) one of whose parts is not allowed, a path outside the working
folders. `dontAsk`, which these steps ran in before, turns exactly those asks into the denial
"Permission to use Bash has been denied because Claude Code is running in don't ask mode": in
2.1.89, 2.1.284 and 2.1.286 the only place that mode denies is where the permission check returned
`ask` (`TGo` in 2.1.286), so in `default` the same calls reach the prompt tool instead. In a
read-only step the edit tools stay removed, and the approval says that Claude Code runs an approved
command outside any sandbox, where it can change files.

`acceptEdits` would run `mkdir`, `touch`, `rm`, `rmdir`, `mv`, `cp` and `sed` in any form without a
rule (a mode check after the deny, ask and allow rules, both 2.1.89 and 2.1.284), and a
`Bash(<command>:*)` deny rule would also deny a listed `sed -n` or `rm -f build/x`, since a prefix
deny rule wins over every allow rule. A writing step with allowed commands therefore never runs in
`acceptEdits`: it allows its edits with an `Edit(//<folder>/**)` rule for the worktree and each extra
writable folder, which both versions accept like `acceptEdits` accepts an edit in a working
directory, after the same deny rules and safety checks. Those seven commands then run only in their
listed forms, and any other form asks: `sed -n 1p README.md` runs, `sed -i …` asks; `rm -f build/x`
runs, `rm -rf src` asks. A prefix still covers what follows it, as every allowed command does (`sed
-n -i …` starts with `sed -n`). The enforcement panel names this mechanism *allowed tools · edit
rules per folder*. No rule can name a folder on a network share, so such a step refuses to start
when its worktree or an extra writable folder is on one, and the enforcement panel shows that
setting as not enforceable. The option's text says what runs (*These commands run, as do known-safe
read-only commands such as ls, cat or git log. Other commands ask you first.*), and the enforcement
panel, the run permissions popover and the pipeline step's commands add "Claude Code also runs
known-safe read-only commands, such as ls, cat or git log, without listing them. Other commands ask
you first."

Each invocation records its launch permissions once, before the CLI starts: the enforced
permissions and how a person is asked about commands outside them (`OutsideCommands`:
`permission_prompt` for Claude Code, `sandbox_approval` for Codex). Launches recorded before agents
asked keep it unknown (their CLIs denied such commands).

A Codex request with an additional permission overlay (`with_additional_permissions`) is not asked
about: such a grant can stay for later commands of the turn, so it cannot be one decision about one
command. The feature is turned off (`features.exec_permission_approvals=false`); if such a request
still arrives, the step stops and its activity says why.

A managed policy with `allowManagedPermissionRulesOnly` makes Claude Code ignore every rule Orchester passes (`--allowedTools`, `--disallowedTools`, the `--settings` ask rule), so none of the above is enforced on such a machine.
