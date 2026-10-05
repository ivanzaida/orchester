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
**Deny** answers `decline` (or, where Codex offers only `cancel`, `cancel`): the command does not run, Codex tells
its model in its own words that it was declined, and the turn goes on; the step does not fail
([Answers](#answers-and-waiting-requests)). A request with an additional permission
overlay (`with_additional_permissions`, off unless `features.exec_permission_approvals` is enabled)
and unsupported request forms fail the step. Two requests are declined at once without a person: a
file change outside the writable roots (`item/fileChange/requestApproval`: edits keep their rules,
only commands are asked about) and a host approval of Codex's managed network proxy
(`networkApprovalContext`, whose grant may outlast the command). A read-only step keeps the
`read-only` sandbox, and its escalations ask as well; the approval says that an approved command
runs outside the sandbox, where it can change files.

Changing permissions from a waiting task's popover applies to the next invocation. When continuing
the task conversation, a session that ran through `codex exec` before App Server became the only
transport resumes through App Server with its original session ID and worktree (`thread/resume` by
id: 0.159.2's `thread/read` found such a session, with `source: exec`, without changing it). A session
Codex has no history of stops the step with *Codex could not resume the earlier session <id>:* and
Codex's reason (`no rollout found for thread id …`). Completed steps, uncommitted work, run limits and the saved pipeline remain
intact. **Apply now** uses the existing checkpoint/restart mechanism. Approval requests and waiting
time are persisted; waiting for a decision does not consume active agent time.

The task's Permissions row shows what the step's current or last invocation launched with, as the runner
recorded it before starting the CLI (`Invocation::permissions`), including whether it asks before MCP actions.
The label carries the qualifier and the value stays bare (design `iPHGE`): *Running with* while it runs, *Last
ran with* after it ended, *Next run uses* before any has launched, then *Builder · Edit files · 3 commands ·
Network off · MCP asks* (*MCP auto* without asking, *Commands ask* when asking before commands). A value breaks
only at its "·" separators; in the "This run" panel it stacks above *change ↗*, elsewhere the link follows it and
moves under it when the line cannot hold both, so it is never cut off. An invocation the runner has created but
not launched yet (it prepares the launch first, the code index included) reads *Running with · starting…*; one
whose CLI never started is skipped, so the row keeps the launch before it; one that ended before launches were
recorded reads *not recorded*. Nothing is inferred from the run's settings or event times.

Saving a change for the next invocation never relabels the running agent: until an invocation launches with
exactly the permissions the change enforces, the row keeps the recorded policy and a notice says *Saved for the
next invocation · pnpm build added*, naming what differs from the launch. An *Apply now* change is pending
(*Applying pnpm build added at the next checkpoint…*, with *Keep for next invocation instead*) until the agent
stops at its checkpoint; then it reads *Restarting with the new permissions…* without that action (the runner
can no longer keep it), or *New permissions not active: Builder did not restart* when the new launch failed, and
it reads as applied only once the restarted invocation has recorded its launch. The popover repeats the line
(*Running with …*) between its header and the timing notice and ends 6 px above its row, scrolling when the
window is too short. For an ended run it shows *Last ran with …*, the settings faded and read-only, why they
cannot be changed, and only *Close*. The Activity panel shows what its own invocation ran with: the row with its
link while the invocation is open, *Ran with …* once it ended, *Ran with · no permissions · it did not start*
for one whose CLI never started.

## App Server requests

The verified Codex 0.159 path for an ordinary MCP tool emits `item/started` with an
`mcpToolCall`, followed by `mcpServer/elicitation/request` with an empty confirmation form.
Orchester correlates its thread, turn and server with exactly one active tool call. A real
disposable write tool was independently approved and denied through the desktop and runner;
approval resumed the same session and wrote once, while denial (then still failing the step) wrote nothing. Since
2026-10-02 a denial declines the call and the turn goes on.

[Codex App Server also documents app tool approvals](https://learn.chatgpt.com/docs/app-server#mcp-tool-call-approvals-apps)
as Accept, Decline or Cancel questions. The local schema names that request
`item/tool/requestUserInput`; the documentation also calls it `tool/requestUserInput`.
Orchester accepts either spelling only for one active MCP call and a blocking, non-secret,
single-choice approval question. Other requests fail the step. Replies never select a session
or permanent grant. No tool-specific implementation depends on Pencil. A real Pencil
`batch_design` call also passed independent approval and denial checks: approval created one
temporary frame and returned its node ID in the same session; denial created nothing. The
temporary frame was removed by that exact ID, preserving the existing design.

A Codex MCP approval request in a step that does **not** ask before MCP actions is declined at once
too: Codex 0.159.2 sends `mcpServer/elicitation/request` for a tool that needs approval even with
`mcp_elicitations` off. Inside the active turn of the step's own thread the runner answers
`{"action":"decline"}` (or the question's decline option) without a person and records the declined call
(`McpToolDeclined`, *declined, not asked* in the activity); Codex tells the agent the call was rejected and the
turn goes on. A request from another phase, thread or turn still fails the step. Each Codex MCP call also ends as an
activity row of its own (`McpToolFinished`, with whether Codex asked first): *approved by you · <server>* when a
person approved it, *asked you · <server>* when it was asked about and did not run, and the server's name alone
when Codex did not ask.

## Tools that run without asking

An MCP server connection can list **Tools that run without asking** (`ExemptTools` in
`orchester_core::connection::mcp`, kept in the server's settings JSON; settings written before have none): tool
names in the order listed, each of ASCII letters, digits, `-` and `_`, at most 64 characters, at most 64 per server,
none twice. A name with other characters is refused, because both CLIs rename such a tool and a rule naming it as
the server writes it would never match. The list applies only to that server's listed tools, and only when the
profile, the step or the run asks before MCP actions; without asking, the server's tools are allowed or declined as
before. It is meant for tools that change nothing. Orchester does not list a server's tools, so the names are typed.
For Pencil: `get_editor_state`, `get_guidelines`, `batch_get`, `snapshot_layout`, `get_screenshot` and
`get_variables`; not `export_nodes`, which writes image files into the folder its call names (`outputDir`).

- **Claude Code** gets one `--allowedTools mcp__<server>__<tool>` rule per listed tool and still no server-wide
  `mcp__<server>` rule, so every other tool of the server reaches the permission prompt. Read in the 2.1.286 binary:
  a rule matches a tool by its exact name, or by server when the rule has no tool part, `*` or a glob there (`Ar`,
  `ZSn`; `mcp__<server>__*` is server-wide); the permission check (`pzt`) reaches the allow rules before it turns an
  MCP tool's own `passthrough` into `ask`, and only an `ask` calls the prompt tool (`hw`). A tool's `readOnlyHint`
  does not change this in `default` mode. Exceptions that still ask: a tool whose `_meta` sets
  `anthropic/requiresUserInteraction`, an organisation's ask ceiling for the server; `allowManagedPermissionRulesOnly`
  ignores the rules altogether (see the end of this document). Claude Code names a tool `mcp__<server>__<tool>`,
  and a server name may hold `__` or end in `_`, so `b__c` of `a` and `c` of `a__b` (or `_x` of `a` and `x` of `a_`)
  share a name. A listed tool whose name another server of the invocation could have gets no rule, since that rule
  would let the other server's tool run unasked: it asks, the activity says so (*… asks after all*, naming the other
  server; `McpExemptionNotApplied`), and its calls are never marked as the listed tool's.
- **Codex App Server** gets `mcp_servers.<server>.tools.<tool>.approval_mode = "approve"` for each listed tool in the
  thread's configuration (`config` of `thread/start` and `thread/resume`, next to the user's servers turned off).
  0.159.2 then runs that tool without `mcpServer/elicitation/request`, while another tool of the same server still
  sends one; the runner never answers a request on a tool's behalf. Checked 2026-10-02 with turns against a local mock
  of the Responses API (no model): one tool set to `approve` ran unasked, another asked; `config/read` lists the
  setting; an unknown mode is refused (*unknown variant*, expecting `auto`, `prompt`, `writes` or `approve`). A tool
  not listed keeps Codex's default mode `auto`, in which Codex itself runs a tool annotated `readOnlyHint`, or with
  both `destructiveHint` and `openWorldHint` false, without asking; Orchester leaves that default as it is.
- **Activity**: each such call records `McpToolNotAsked` and reads *not asked · exempt tool of <server>*, the wording
  of the command categories design's marks: after the exit and duration of Claude Code's command row, and as a row of
  its own for Codex, which reports no command for an MCP call. It is no approval: no `tool_approvals` row, nothing in
  *Needs you*.
- **Screens** (no frame draws them): the MCP server dialog edits the list one name per line under the transport's
  fields; the Connections inspector lists the names after the variables, or *None*; while the profile asks before MCP
  actions, its enforcement panel has a row per server (*pencil: 6 tools run without asking*, with the CLI's rule), and
  the pipeline step inspector repeats the line under *Ask when a tool needs approval*.

## Orchester's tools

Built 2026-10-05 on `claude/orchester-tools` (`orchester_core::orchester_tools`), from the user's decision of that day:
an agent's shell lines cannot be read by Orchester, so each one outside the allowed commands asks a person (59
approvals in 1.5 hours of Orchester developing itself). One set of typed tools is defined in core, with its schema,
its approval policy and its execution, and served to both CLIs by the relay Orchester already runs for each
invocation; a native agent can later declare the same schemas in its own request and run the same calls (P1d).

| Tool | Input | Access class | Writing step | Read-only step |
| --- | --- | --- | --- | --- |
| `git_status` | none | read-only | runs | runs |
| `git_diff` | `path?`, `staged?` | read-only | runs | runs |
| `git_log` | `path?`, `count?` (1–200, 20) | read-only | runs | runs |
| `git_show` | `revision`, `path?` | read-only | runs | runs |
| `git_add` | `paths` (1–100) | writes the worktree | runs | not offered |
| `git_commit` | `message` | writes the worktree | runs | not offered |
| `read_outside_file` | `path` (absolute), `offset?`, `limit?` (≤ 10 000, 2000) | outside | asks | asks |
| `run_project_tool` | `name` | outside, network | asks (network on only) | asks (network on only) |

- **Schemas** are strict (`additionalProperties: false`, each property typed and bounded); a call is read into its
  type with unknown fields refused, and an invalid call is answered with an error result that says why.
- **Approval, held by the runner.** A call of a tool that asks runs only once approved, whatever the CLI does: an
  approval through Claude Code's permission prompt leaves a grant for one call of that tool, and a call that arrives
  without one is asked about by the runner itself before it runs (`orchester-call-<n>` requests). Codex is told to
  run every tool (`approval_mode = "approve"` in the thread's configuration), so the runner asks for each call that
  asks; whether Codex 0.159.2 would send its own request for a `prompt` tool while `mcp_elicitations` is off is not
  relied on. A call that asks reaches a person whether or not the step asks before MCP actions; rules of the run and
  auto-approval answer it as they answer an MCP call, but auto-approval never runs a project's program. *Approve
  similar* covers the tool for the run, whatever its arguments (one approved read covers later reads); *Always allow*
  is unavailable (*Orchester's own tools have no list of tools that run without asking yet*).
- **Execution.** The runner runs every call off its async workers: a program and its arguments, never a shell line,
  in the run's worktree with the environment the agent's own programs get (no secret). Each value is one argument of
  its own after the options (`--literal-pathspecs`, `--` before paths, `--end-of-options` before a revision, which
  may not start with `-` or hold whitespace or `:`; paths are relative, without `..` or `:`); a commit message goes
  to git's standard input (`-F -`). Git is pinned to the worktree and its own git folder (`--git-dir`, read from the
  repository, `--work-tree`), so a rewritten `.git` file cannot lead it to a configuration of the agent's making;
  the repository's own configuration applies, filters included (as for the runner's checkpoint commits). Git also
  gets the native agent's hardening, no diff or textconv program, an absolute empty hooks folder the runner owns
  (`<temp>/orchester-mcp/<project>/no-hooks`) with `--no-verify`, no signing and no signature checks. Output is read
  as it comes and kept within the cap: the first 128 KiB for git (60 seconds), the end of a project tool's output
  within its own limits; the answer starts with the exit code and has the invocation's MCP secrets removed.
- **`read_outside_file`** reads the whole disk but a deny list, rather than an allow list of dependency folders:
  where toolchains keep sources differs by toolchain and machine, and Orchester names no toolchain's folders. Refused
  are the shared credential rule's locations (`CommandScope::holds_credentials`; published sources such as a package
  registry's `src` are not credentials), anything in a `.git` folder, and Orchester's own state wherever it lies: the
  whole `<temp>/orchester-mcp` tree (every project's private files with MCP secrets and tokens), any `.orchester`
  folder but the run's worktree, any `orchester.db*` file and the folder of Orchester's programs; device, stream,
  verbatim, drive-relative and network paths; a folder or pipe. The file is read as it comes: numbered lines, each
  cut to 16 KiB, at most 256 KiB, with where to read on. Known limits: the path is checked, then opened, so a link
  swapped in between by another program could lead elsewhere (what is opened must still be a file); a mapped network
  drive (`Z:\`) or a mounted share (`/Volumes/…`) reads as a local folder, also in a step without network.
- **`run_project_tool`** is built in core but offered to no agent: the Tools library has no *offered to agents*
  setting and no typed parameters, and its access rules were made for pipeline steps. When offered, it is not offered
  without network (the program runs outside every sandbox) and always asks.
- **Claude Code** gets the tools from the permission prompt's relay (`--claude-permission-prompt`), which lists them
  after `approve`; every tool that runs gets an allow rule (`mcp__Orchester__<tool>`), so the others reach the
  prompt. Claude Code 2.1.286 removes only the prompt tool from the model's tools (`permissionPromptToolName` is
  filtered by exact name). **Codex** gets the same relay (`orchester-runner --orchester-tools`, no `approve`) as the
  server `Orchester` of its configuration, the runner's address in `env`, the token through `env_vars` (kept out of
  the agent's shell commands) and a `tool_timeout_sec` long enough for a person's decision. A user's own server named
  `Orchester` refuses the step. The relay handles one request at a time, so a call waiting for a person holds the
  invocation's other calls of the relay.
- **Prompt.** After the *Files and commands* part, a new session that gets the tools is told in a short paragraph of
  its own (*# Orchester tools*) to use them for git and for files outside the worktree.
- **Checked** with the fake CLIs only (`cargo test -p orchester-runner --features test-agent --test orchester_tools
  orchester_tools`). Not checked with the real CLIs: how large answers fare against Claude Code's MCP output limit
  (`MAX_MCP_OUTPUT_TOKENS`).

## Answers and waiting requests

Built 2026-10-02 from the user's decisions of that day (design doc, "Approval choices and waiting requests";
frames `kpuQj`, `NV6xJ`, `XKja4`, `bjrgu`, `G0rzJy`, `ZkOPf`). A person answers a waiting request with one of
`orchester_core::run::ApprovalAnswer`, for commands and MCP calls alike, through `Command::DecideToolApproval`
(IPC protocol 12; 13 adds `Rejection::ToolApprovalAnswered`, below). Towards the agent's CLI every answer still answers that one request: no session grant,
`acceptForSession`, `updatedPermissions` or execpolicy amendment is ever given. What a wider answer promises
Orchester keeps itself, and before a request waits for a person the runner answers it itself when that covers it
(`step/answer.rs`), recording who answered (`tool_approvals.decided_by`, `DecidedBy`) and marking the call in the
activity (`AgentEvent::ToolDecided`):

- **Approve once**: this request only.
- **Approve similar in this run** adds a run rule (`approval_rules`, `RuleScope`). For a command it names the
  program and subcommand of its asking part in that part's category (`cargo add` in Network, `cp` in Outside write;
  `permission::category`); a later command is answered when a rule covers one of its parts and every other part is
  covered too or is a plain read (Read-only, reading no credential location, installing nothing). A rule never
  covers a part that rewrites history, deletes outside the worktree, reads a credential or is Unknown, and covers a
  part that reaches outside the worktree only in the folders the approved part reached (`cp … /tmp/x` covers
  `cp … /tmp/y`, not `cp … ~/AppData/…`); `ssh`, `scp` and `rsync` make none. Unavailable for a command Orchester
  cannot read to the end, with several asking parts that are not alike (several `sed -i` edits in the worktree make
  one rule), that rewrites history or deletes outside, or reads a credential. For an MCP call it names the server and
  the tool, any arguments. Rules are read at each request, so they apply at once, in the current invocation too;
  later answers are marked *approved by similar rule*. The run's Summary lists them with **Revoke** (the desktop
  marks the rule revoked; the next request asks again); calls approved before keep their mark. Rules end with the
  run and survive a runner restart.
- **Always allow…** shows a second step with exactly what is added. A command's prefix (program and subcommand, or
  the whole command) goes into the profile's Allowed commands: the runner publishes the profile's latest version
  with it as the next version and adds it to an unsaved draft, which is then based on that version
  (`ProjectDb::publish_allowed_command`, one transaction). The run keeps its pinned version and keeps the prefix as
  a rule (*always allowed*), until it ends. It is unavailable for a profile that asks before every command or whose
  CLI keeps no list (Codex), for History / destructive and Unknown commands and those that read a credential, and for a
  prefix that would also allow a more dangerous form: only builds, tests and installs (which run the worktree's or
  the packages' code whatever their arguments) and the git subcommands that stay in the repository (`git status`,
  `git add`, …) offer one; `git config`, `gh`, `git fetch`, `curl`, `cat`, `rm`, … do not. An MCP tool goes into its server's Tools that run without asking (a
  connection setting, saved at once); later calls of the current invocation, whose CLI read the list when it
  started, are answered by the runner (*always allowed*), and later invocations get the CLI rule.
- **Approve everything in this run** turns on the run's auto-approval (`runs.auto_approve`), below.
- **Deny**: the call does not run; the agent is told and continues (Claude Code: the message; Codex: `decline`).
  The activity reads *denied by you · the agent continued* with what the agent was told. Checked 2026-10-02 with
  the real CLIs against local mocks of the model APIs (no model; scripts in `.tmp/approval-choices/`): Claude Code
  2.1.286 handed the agent the message as an error tool result, ran nothing, and the turn ended with the model's
  final message (`permission_denials` lists the call); Codex 0.159.2 told its model `user rejected MCP tool call`
  for an MCP call and `exec_command failed: CreateProcess { message: "Rejected(\"rejected by user\")" }` for an
  escalated command (`run::codex_denial`), ran nothing, and the turn completed.

**Auto-approval** (user decisions 2026-10-02 and 2026-10-03) is a run's own choice (*Approve everything in this run*, the switch in
the run's Summary, *Stop* there, under the task heading or in the status bar) or, while the run has none, the
profile setting *Approve automatically (don't ask me)* (`AgentPermissions::auto_approve`, part of the profile
version). It approves only commands it knows to read or stay in the worktree, and MCP tool calls, and never what
`run::auto_verdict` leaves to a person: a command that reaches the network, a write, move or delete outside the worktree and its writable folders (the
temporary folder too: Run temp is not told apart yet), a read of a credential location (dot files and folders of
the home folder such as `~/.claude.json`, `~/.codex`, `~/.ssh`, everything under `AppData` and `Library` but
the temporary folder (CLI tokens, the Windows credential folders, the macOS keychains, browser and editor
profiles), or a credential store program), an install of software or packages (`npm install`, `cargo add`,
`pip install`, `npx`, global installs, …), History / destructive commands (whose category cannot be allowed) and
anything not read to the end. What the step's permissions refuse never reaches a person, so it stays refused.
Answers read *auto-approved · <category>* (or the MCP tool). The per-category Allow / Ask me / Deny policy of
"Command categories and approval policy" is not built, so no category can be set to Deny yet.

**Waiting requests.** The dialog shows one request at a time, oldest first, with *1 of 3 waiting* and ‹ ›.
*Decide later* (or Esc) closes it and answers nothing: the agent keeps waiting with its active time stopped, the
request stays in *Needs you* (which counts each waiting request, the dialog open or not), on the task page (*Review
request*) and in the status bar (*3 approvals waiting · … · Review*). While a request put off waits, a new one does
not open the dialog by itself; a click on the backdrop does nothing.

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
Codex's standard error is kept in the step's activity as raw lines, without colours and without
Codex's warning about unknown settings (which names the user's settings and files), with secret values
redacted before a line is cut, at most 1000 lines of at most 4000 characters (256 KiB) per invocation;
the activity then says how many more lines it left out. A turn's usage adds the `last`
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

Project Environment variables reach agents and command tools as written; nothing is rewritten per
worktree, whatever a variable names. A build tool that writes into the worktree by default keeps
its output there, and it goes away with the worktree.

## Interrupted decisions

An approval is tied to one live process and JSON-RPC request. If the runner or desktop restarts
while a tool waits, or after approval was delivered before the turn completed, Orchester records the
interruption and does not automatically resume the
run. The task page shows the interrupted run for a person to review before choosing to continue.
The old decision cannot approve a later tool call.

A request waits only while a live agent holds it. The runner ends one whose agent can no longer receive an
answer (state `interrupted`, read as *ended without an answer*): its invocation's end interrupts what the worker
held; a run whose task stops executing it (it ends, stops, fails or waits for a person) ends the requests of it it
reads as still waiting; an attaching runner ends the waiting requests it reads in every run it does not execute,
since no agent of an earlier runner is at work (each by id, `ProjectDb::end_waiting_tool_approval`, so a request a
new agent records meanwhile is not touched); and an answer to a request no live agent holds (no task executes the
run, the step's worker ended, or the worker waits on another call) ends it and is refused with
`ToolApprovalNotAwaited`. An answer to a request answered already (a second click, or Orchester answered it) is
refused with `ToolApprovalAnswered` (IPC protocol 14), which the desktop does not read as an end. The desktop drops a request the
runner refuses that way from the dialog and the inline card at once, and says it ended (design doc, "Approval
choices and waiting requests").

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
server's own approval hints to the prompt; only the tools a server lists to run without asking get an
`mcp__<server>__<tool>` allow rule ([Tools that run without asking](#tools-that-run-without-asking)).

**Approve once** answers `{"behavior":"allow","updatedInput":{}}`: the call runs with its own input
(2.1.89 keeps the input when `updatedInput` is empty), no `updatedPermissions` rule is returned, and
the input is never echoed back. **Deny** answers `{"behavior":"deny","message":"…"}` with what the agent is told
(`orchester_core::run::denial_message`: *Denied by the person reviewing this run. Continue without it.* for a
command, *The person reviewing this run denied <server> <tool>. Continue without it.* for an MCP call) and no
`interrupt`, so Claude Code hands the message to the agent as the tool's result and the turn goes on, as for
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

Claude Code asks about a command line whose parts it cannot match one by one: a chain (`git status && git diff`), a
`cd` into the worktree before the command, a variable the line sets and uses (`f=src/lib.rs; sed -n 1,20p $f`).
Orchester answers such a request itself when every part would run on its own (`run::permitted`, recorded as
`DecidedBy::Permissions` and marked *allowed by the profile · <category>*; the run's Summary counts it with
*Always allowed*): listed in the step's allowed commands and naming nothing outside the worktree, or a known-safe
read inside the worktree (`ls`, `cat`, `head`, `tail`, `wc`, `grep` without `-R`, `rg`, `sort`, `uniq`, `find`
without `-exec` or `-delete`, `sed` whose script only prints, selects or changes lines without `-i`, `awk` whose
program uses only the names an allow-list holds, the git subcommands that only read). The classifier reads such
lines to their end: a `cd` to one folder named plainly in the worktree (written `/h/…`, `H:/…` or `H:\…`) is
dropped and the rest placed from that folder; a `cd` with an option, `-`, a second operand (zsh's `cd old new`) or a
target that cannot be told leaves where later parts run unknown, and every part reads the folder it runs in. A
literal assignment (`NAME=value`, no expansion, a name of one letter and digits, in the shell itself) is read
into its later `$NAME` and `${NAME}`. A glob names each path it matches now, links followed (`cat ke?` through a
link to `~/.ssh` reads a credential). A part that writes through the shell (`sed -i`, a heredoc, `tee`), redirects
into a file (`>`, `>>`, `&>`, `>&file`; `>|`, here-strings and process substitutions are not read), runs in the
background, reads outside the worktree or a credential, installs or is not read to the end still asks, with
Approve similar where the existing rules offer it. The step's prompt tells Claude Code and Codex agents how to work
with the tools they have (*Files and commands*: Claude Code's file tools, the edit tools only in a writing step,
one bare command per call; Codex's `apply_patch` in a writing step, no `cd`), so they rarely send such lines.
Reads of the sources Cargo unpacks (`~/.cargo/registry/src`) and of the toolchains' library sources
(`~/.rustup/toolchains/*/lib/rustlib/src`) are reads outside the worktree, not credential reads; the rest of
`~/.cargo` and `~/.rustup` stays a credential location, and so does any path through a Windows share or a `\\?\`
prefix, which can name the home folder under another name.

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
