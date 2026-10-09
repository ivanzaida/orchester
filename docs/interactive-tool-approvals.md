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
- **Codex App Server** gets `mcp_servers.<server>.tools.<tool>.approval_mode = "approve"` for each listed tool as an
  override of its process (`-c mcp_servers.<server>.tools={"<tool>"={approval_mode="approve"},…}`), next to the
  server's own. Not in the thread's configuration (`config` of `thread/start` and `thread/resume`), which holds the
  user's servers turned off only, each by its dotted key (`"mcp_servers.<server>.enabled": false`): in 0.159.2 a
  thread's `mcp_servers` table, even an empty one, replaces the table of the process's overrides, so the step's
  servers and Orchester's are gone from the thread (the step then failed with *omitted an MCP server selected by
  this agent profile*, or its agent worked without Orchester's tools), and a table for a server only overrides
  define refuses the thread with *invalid transport in `mcp_servers.<server>`* (checked 2026-10-07 and 2026-10-08
  with `config/read`, `thread/start` and `mcpServerStatus/list`).
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

Second version (M12, the user's decision of 2026-10-09, branch `claude/agent-tools`): no tool takes a project or a
run (the relay belongs to one invocation, worktree paths are relative to its worktree); Orchester's git tools,
`write_file`, `read_outside_file` and the old `search`/`edit_file` schemas are gone (agents use git through their
shell); the worktree is read, searched, edited by line range and created in; files outside it are read and searched
by `read_external_file` and `search_external_file`.

| Tool | Input | Access class | Writing step | Read-only step |
| --- | --- | --- | --- | --- |
| `read_file` | `filename`, `start_line?`, `length?` (≤ 10 000, 2000) | read-only | runs | runs |
| `list_files` | the native agent's (`pattern?`, `path?`) | read-only | runs | runs |
| `search_file` | `term` (literal, one line, ≤ 1000 characters), `limit_files?` (≤ 200, 20) | read-only | runs | runs |
| `edit_file` | `filename`, `start_line`, `end_line`, `expected`, `content` | writes the worktree | runs | not offered |
| `create_file` | `filename`, `content` | writes the worktree | runs | not offered |
| `delete_file` | `filename` | writes the worktree | runs | not offered |
| `move_file` | `from`, `to` | writes the worktree | runs | not offered |
| `find_references` | `name` (an identifier) | read-only | runs | runs |
| `find_symbol` (with the code index) | `name`, `kind?`, `include_tests?`, `include_consts?` | read-only | runs | runs |
| `search_external_file` | `base_directory` (absolute), `term` | outside | asks; runs in the run's sources | asks; runs in the run's sources |
| `read_external_file` | `filename` (absolute), `start_line?`, `length?` | outside | asks; runs in the run's sources | asks; runs in the run's sources |
| `run_check` (with checks) | `name` (one of the step's checks) | outside, network | by the check's access rules (network on only) | as a writing step |
| `list_skills` (with skills) | none | read-only | runs | runs |
| `read_skill` (with skills) | `name`, `file?` (a skill file path; none or `SKILL.md` for the skill's own), `offset?`, `limit?` (≤ 10 000, 2000) | read-only | runs | runs |
| `get_task` | none | read-only | runs | runs |
| `save_memory` | `kind`, `text`, `key?`, `tags?`, `importance?`, `scope?` (`task`, the default, or `project`) | records | runs | runs |
| `report_progress` | `text` (one line, ≤ 200 characters) | records | runs | runs |

- **Schemas** are strict (`additionalProperties: false`, each property typed and bounded); a call is read into its
  type with unknown fields refused, and an invalid call is answered with an error result that says why.
- **Worktree files** (`orchester_agent::FileTools`, confined to the step's `FileScope` as before: the worktree, the
  extra writable folders writable by `edit_file` and `create_file`, the code index and skills read-only; `.git`,
  `.orchester` and links that lead out refused). `read_file` reads numbered lines from disk (the code index holds no
  file contents) and records a read stamp. `edit_file` replaces lines `start_line..=end_line` with `content` only when
  the file was read in this invocation, has not changed since (the stamp: length, modified time, hash) and those lines
  are exactly `expected` now (compared without line endings); otherwise nothing is written and the answer shows what
  the lines hold now. `end_line = start_line - 1` inserts without replacing. The file's CRLF, byte order mark and a last
  line without an ending are kept; the write is atomic and stamps the result, so the next edit needs no new read.
  `create_file` refuses a file that exists and makes missing folders. `search_file` searches for literal text (a term
  in lower case matches any case) with ripgrep (`--fixed-strings --smart-case --sort path`, `.gitignore` respected,
  hidden entries skipped; without ripgrep a folder walk), lists each file with up to 30 matching lines, at most
  `limit_files` files, and says when it was cut; with the code index, a name's definitions come first.
- **Outside the worktree.** `read_external_file` and `search_external_file` reach the whole disk but a deny list,
  rather than an allow list of dependency folders: where toolchains keep sources differs by toolchain and machine, and
  Orchester names no toolchain's folders. Refused are the shared credential rule's locations
  (`CommandScope::holds_credentials`; published sources such as a package registry's `src` are not credentials),
  anything in a `.git` folder, and Orchester's own state wherever it lies: the whole `<temp>/orchester-mcp` tree
  (every project's private files with MCP secrets and tokens), any `.orchester` folder but the run's worktree, any
  `orchester.db*` or `orchester-server.db*` file and the folder of Orchester's programs; device, stream, verbatim,
  drive-relative and network paths; a folder or pipe; a file with a NUL anywhere up to the last line read. A read is
  numbered lines, each cut to 16 KiB, at most 56 KiB, with where to read on (`start_line`). A search lists the
  checked folder, keeps only the files a read would reach (each checked; the ones left out are neither counted nor
  named) and searches those, at most 20 files. A path the tools never reach is refused before anybody is asked. A link
  on the way whose target is a network or device path is read, never followed, and the path is refused before it is
  resolved, so the server is not contacted (`permission::link_targets`). Known limits: a path is checked, then
  opened, so a link swapped in between by another program could lead elsewhere (what is opened must still be a
  file); a mapped network drive (`Z:\`) or a mounted share (`/Volumes/…`) reads as a local folder, also in a step
  without network; on macOS an automounter folder (`/net/<host>` where autofs is on) mounts a share when a path in it
  is resolved.
- **Sources** (`orchester_core::source`): folders and files outside the worktree an agent may read without asking,
  absolute paths listed on the project and on a task (`project_sources`, `task_sources`, this machine's paths, never
  shared; set through `ProjectDb::set_project_sources` and `set_task_sources`), on a pipeline and on its steps (in the
  pipeline's versions, `PipelineSources`, set with `Pipeline::with_sources` and kept by `PipelineEdit`; a removed
  step's go with it). A run snapshots their union when it starts, in its run snapshot (`RunSnapshot::with_sources`:
  the project's, the task's, the pipeline's, then each step's, each path once with where it was listed first), so
  later edits apply to new runs; every step of the run reads all of them. A call of `read_external_file` or
  `search_external_file` whose resolved file or folder is a source or lies in one runs without asking: the runner
  answers Claude Code's permission prompt itself, and runs a Codex call at once; what the tools refuse anywhere stays
  refused in a source too. The *# Orchester tools* paragraph names the run's sources. Their screens (Project
  settings, New task's *Attach context*, the pipeline settings and step inspector; `data/orchester2.pen`, layers
  *Sources* and *Pipeline settings*) are not built: until then sources are set through the store API (Orchester's
  domain MCP server of `crates/workspace` does not exist on master yet).
- **Approval, held by the runner.** A call of a tool that asks runs only once approved, whatever the CLI does: an
  approval through Claude Code's permission prompt leaves a grant for one call of that tool with exactly the
  arguments the person saw, and a call that arrives without one is asked about by the runner itself before it runs
  (`orchester-call-<n>` requests). Codex is told to run every tool (`approval_mode = "approve"`, an override of its
  process), so the runner asks for each call that asks. A call that asks reaches a person whether or not the step
  asks before MCP actions; rules of the run and auto-approval answer it as they answer an MCP call, but auto-approval
  never runs a project's program. A request for a call outside the worktree names, as its target, the folder the call
  reads in (a read's file's folder, a search's folder, resolved), and *Approve similar* keeps its rule to that folder:
  it answers every later read and search there and below it (`RuleScope::SimilarMcpTool` with a `folder`; rules made
  before keep covering their tool everywhere). The approval tile and the Summary's rule name that folder and say the
  rule covers reads and searches. It is unavailable for a folder too broad to approve as a whole
  (`CommandScope::is_broad_folder`, paths resolved and compared without case): a drive or file system root, a mount
  point (a folder on another device than its parent on macOS, a folder that is still a reparse point once resolved on
  Windows), a folder fewer than two levels below one of those, the user's home folder or another user's (a folder in
  the profiles folder: the home's parent, such as `C:\Users`, and `/Users` and `/home`), a folder that holds one, or
  one fewer than two levels below one.
  *Always allow* is unavailable (*Orchester's own tools have no list of tools that run without asking yet*). A call
  that asks nobody (a read in the sources, a check whose rules approve no call) runs without leaving a grant.
- **Execution.** The runner runs every call off its async workers, in the run's worktree with the environment the
  agent's own programs get (no secret); a project tool is a program and its arguments, never a shell line, its output
  read as it comes and kept within its own limits; the answer has the invocation's MCP secrets removed and stays within
  60 KiB, with a note on how to narrow the call (`start_line` and `length`, `limit_files`, a longer term or a path).
  To list the files changed since the code index, git is pinned to the worktree and its own git folder (`--git-dir`,
  read from the repository, `--work-tree`), with the native agent's hardening, an absolute empty hooks folder the
  runner owns and submodules off.
- **`run_check`** (formerly `run_project_tool`) runs a check: a command tool of the Tools library that the run pinned
  and whose access rules let the run's pipeline and the step call it (`AccessPolicy::allows`), named by its slug, with
  its fixed arguments, in the worktree; the answer is its exit code and the end of its output within the tool's own
  limits. A check whose rules approve no call (`ApprovalMode::Never`) runs without asking; any other asks
  (`StepTools::call_asks`), and Claude Code gets an allow rule for `run_check` only when no check asks. A request
  names the check, and *Approve similar* keeps its rule to that check (`RuleScope::SimilarMcpTool` with a `check`). A
  check approved once per run (`ApprovalMode::OncePerRun`) asks at its first call; *Approve once* then also adds
  that rule, so its later calls in the run are answered by it. For a check approved before every call
  (`ApprovalMode::EveryCall`), or one the step is not offered, *Approve similar* is unavailable. It is not
  offered without network (the program runs outside every sandbox), and auto-approval never runs one. No free-form
  command: the checks have no typed parameters.
- **The run's records.** `get_task` answers the run's task as the run started it (title, brief, acceptance
  criteria), the plans approved in the run, and the person's decisions and notes there (changes requested on a plan,
  answers to the agents' questions, the person's messages to the run). `save_memory` keeps a record in the task's
  memory (or the project's, with `scope: "project"`, an addition to the plan's signature), active or proposed as the
  project's memory setting says for that scope, with the agent's provenance; a `key` replaces the active record with
  that key once the note is active; a note that holds a project secret's value is refused whole (`SecretScreen`).
  `report_progress` keeps one line per step execution (`step_progress`, migration; the latest replaces the one
  before), the invocation's secret values removed; the Summary that shows it is not built. `write_artifact` is not
  built: the project database has no `artifacts` table yet (`docs/architecture/storage.md` lists one: run, name,
  blob, and the Artifacts design wants the step execution and revision it was made at too).
- **Deletions and moves.** `delete_file` removes one entry of the worktree, `move_file` renames one to a path of the
  worktree where none is (making its folders); the extra writable folders are not theirs. Both change the entry they
  checked, a link itself and never its target (`orchester_agent` `files::entry`): on macOS the folders from the
  worktree down are opened without following links and the entry is removed or renamed relative to its folder
  (`unlinkat`, `renameat` with no-replace); on Windows the entry is opened with `FILE_FLAG_OPEN_REPARSE_POINT`, the
  path the handle reached must be the one checked (`GetFinalPathNameByHandleW`), and it is deleted or renamed through
  that handle (`SetFileInformationByHandle`, never replacing), the target's folder held open without sharing deletion.
  Both show in the run's changes (the transcript reports the deletion, and the move as a deletion and an addition).
  The moved file is read again before it is edited.
- **`find_references`** lists a name's definitions from the code index (marked when their file changed since the
  index), then the worktree's files that use it as a whole word in exactly its case (ripgrep `--word-regexp
  --case-sensitive`, or the walk), at most 50 files; without the index it says so.
- **Claude Code** gets the tools from the permission prompt's relay (`--claude-permission-prompt`), which lists them
  after `approve`; every tool that runs gets an allow rule (`mcp__Orchester__<tool>`), so the others reach the
  prompt. Its own `Read`, `Edit`, `MultiEdit`, `Write`, `NotebookEdit`, `NotebookRead`, `Grep`, `Glob` and `LS` are
  added to `--disallowedTools` whenever it gets the relay (the user's decision of 2026-10-09: the prompt names
  Orchester's tools and the CLI's own are taken away; its shell stays for builds, tests and git). Claude Code 2.1.286 removes only the prompt tool from the model's tools
  (`permissionPromptToolName` is filtered by exact name). **Codex** gets the same relay (`orchester-runner
  --orchester-tools`, no `approve`) as the server `Orchester` of its configuration, the runner's address in `env`, the
  token through `env_vars` (kept out of the agent's shell commands) and a `tool_timeout_sec` long enough for a person's
  decision. Its shell cannot be taken away, so it is steered by the prompt alone; the live run measures how often it
  still reads or edits through the shell. A user's own server named `Orchester` refuses the step. The relay handles
  one request at a time, so a call waiting for a person holds the invocation's other calls of the relay. Codex 0.159.2
  starts the server once for the thread and once more for each `mcpServerStatus/list`, at about the same time (checked
  2026-10-08), so the runner serves the two Codex relays that show the token, each on its own connection, and closes
  the port after them: a later connection, even with the token, is refused. Once Codex's inventory was checked, the
  port waits at most 10 seconds for a relay still to come (Codex may answer the list before the thread's relay came);
  a handshake under way when it closes still counts, and when fewer than two came the step's activity says that the
  agent may have worked without Orchester's tools. Claude Code's permission prompt relay is served alone. Each
  connection shows the token on its own, at most eight at once (more wait in the port's backlog), so a silent one
  holds no relay back; a connection without the token never closes the port.
- **`list_skills` and `read_skill`** (added 2026-10-08, branch `claude/skill-tools`): Codex agents could not read
  their skills. The prompt gave each skill's `SKILL.md` in the folder placed for the step execution,
  `<repo>/.orchester/skills/<execution>/<slug>/`, which the reads outside the worktree refuse as Orchester's state, so
  each run stopped to ask for a shell read. `read_file` does reach that folder by its absolute path (the file tools
  read the skills folder, read-only) and the repository's skills as worktree files. Both tools are offered, and run
  without asking, in a step that has skills; the runner serves exactly what it placed for this step execution (the
  run's pinned skills that reach the agent and the internal `orchester-codebase-index`), and, under the project's
  *Repository agent files*, the repository's skills the agent's CLI itself loads (`AgentFilesMechanism`): Codex's of
  `<worktree>/.agents/skills` and `.codex/skills` while it is on, none while it is off; Claude Code none in either
  state (`--setting-sources ""`), so the tools never widen what the setting lets a CLI load. The repository's skills
  are found as Codex finds them (every `SKILL.md` at any depth, through links), but only within the worktree: a
  folder or `SKILL.md` that resolves out of it or into its `.git` or `.orchester` is left out, a link loop is read
  once, the search stops at 8 folders deep, 500 folders and 100 skills, a `SKILL.md` is read only once it resolves
  to a regular file of its folder (by its resolved path; a pipe or device is never opened), and nothing the search
  meets stops the agent's start. A skill whose name is not one line of plain text (at most 200 characters, no
  control character or edge whitespace) is left out without its name. `list_skills` names each skill with its source
  (`orchester` or `repository`), description and reference files; a name is offered once (Orchester's first), and
  what is left out is listed by where it lies in the worktree, never by a path outside it. `read_skill` reads only the
  skill's own regular files, by the same rule the list uses: Orchester's skill its `SKILL.md` and the files its
  version lists, a repository skill its folder's files but those of a nested skill, a `.git` entry, the worktree's
  `.orchester` or a link that leads out of the folder or the worktree. Its `file` is a skill file path (no `..`,
  drive, absolute path or `\`); the answer is numbered lines within `read_external_file`'s caps, redacted like every
  answer. Claude Code gets an allow rule for both when it gets a skills folder, which then holds every skill it is
  offered. The native agent does not get Orchester's tools yet (P1d) and keeps reading its skills with its own
  `read_file` by the paths its prompt lists.
- **Prompt.** The *Files and commands* part names the worktree file tools as each CLI sees them (Claude Code:
  `mcp__Orchester__read_file`, …, and that its own Read, Edit, Write, NotebookEdit, Grep and Glob are not available;
  Codex: the tools of the `Orchester` server, not the shell's `cat`, `rg`, `sed -i` or heredocs), and how `edit_file` is called (the
  lines to replace and, as `expected`, what they hold now). After it, a new session that gets the tools is told in a
  paragraph of its own (*# Orchester tools*) to use the shell for builds, tests and git, not to read or search files;
  to read and search outside the worktree with `read_external_file` and `search_external_file` (at once in the run's
  sources, which it names; elsewhere a person is asked first); the other tools of the server it has; and, with skills, to read them with `read_skill` by name, never by path. The *# Skills* part then
  names each skill and its description without a path and says to read it with `read_skill`; an agent without the
  tools (Claude Code without its permission prompt, the native agent) still gets each `SKILL.md`'s path.
- **Checked** with the fake CLIs only (`cargo test -p orchester-runner --features test-agent --test orchester_tools
  orchester_tools`, `--test orchester_file_tools orchester_file_tools`, `--test orchester_skill_tools
  orchester_skill_tools`, `--test orchester_run_tools orchester_run_tools`); sources also with `cargo test -p orchester-core --test run sources` and `cargo test -p
  orchester-store --test sources sources`. Not checked with the real CLIs: whether Claude Code and Codex call each tool as the prompt
  asks, how often Codex still reads or edits through its shell, and how large answers fare against Claude Code's MCP
  output limit (`MAX_MCP_OUTPUT_TOKENS`).

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
| the user's MCP servers | each turned off for the thread by name (`config` of `thread/start`/`thread/resume`: the dotted key `"mcp_servers.<name>.enabled": false`), the names read with `config/read` first; a table merges with the user's, so `mcp_servers={}` would not remove them, and it replaces the servers of the process's overrides, the step's own and Orchester's. A dotted key cannot name a server whose name has a dot (Codex splits it at every dot), so such a server of the user's configuration refuses the step before its thread starts and says to rename it. A server the step chooses that is not connected is named in the step's failure (it asks before MCP actions) or its activity, with Codex's state and reason. The thread's inventory must then show every server the step does not choose `disabled`, or the step fails. A server of the user's own configuration named like one the step chooses would merge its `command`, `args` or `env` into it (Codex merges the tables of its layers), so the step stops and says to rename one. |
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

While *Repository agent files* is off, the runner finds every `SKILL.md` of both folders as Codex does
(at any depth, through links) to disable it, and the search fails closed, because Codex follows links
when it loads skills and would load a skill the search missed (`orchester_core::agent_files::skills`,
added 2026-10-08, branch `claude/skills-off`). Every path that reaches a folder is followed, and each
`SKILL.md` is disabled by each path through the worktree that reaches it and by its resolved path.
Codex 0.159.2 (checked on Windows on 2026-10-08 with `skills/list` of `codex app-server` under a scratch
`CODEX_HOME`, without signing in or a model turn) lists a skill reached through a junction by its
resolved path, lists a folder reached by two paths once, and `skills.config` disables a skill by either
path; listing both keeps working should another version match only one. The same check showed that
Codex passes over hidden entries (`.hidden`, `.git`) and looks 6 folders deep. The search still walks
hidden folders, in case another version loads them, but never a `.git` entry, which Codex skips and
whose folders (a nested repository's objects) would only use up the bounds. The agent does not start,
and the step says why by where the cause lies in the worktree (never where a link leads), when:

- a folder or `SKILL.md` resolves out of the worktree (a link, or a junction on Windows), which is not
  walked: *Repository agent files are off, but `.agents/skills/x` leads out of the worktree, so Codex
  could load skills from it. Remove the link or turn the project's Repository agent files on.*;
- a folder in the worktree cannot be read: *… but `<path>` cannot be read (<error>), so Orchester
  cannot disable the skills Codex could load from it. Make it readable or …*;
- the search passes its bounds, the same as `list_skills` uses: a folder more than 8 folders below its
  skills folder (*… `<path>` lies more than 8 folders deep in its skills folder, so Codex could load
  skills there that Orchester does not look for. Make the skills folders smaller or …*), or more than
  500 folders below the skills folders, each path to a folder counted (*… the skills folders lead to
  more than 500 folders, each path to a folder counted, …*). A link loop within the worktree therefore
  runs into the depth bound and refuses.

The native agent, which Orchester gives the repository's skills while the setting is on, uses the same
search and does not start for the same causes (*cannot load the repository's skills: `.agents/skills/x`
leads out of the worktree*). It lists each skill once, by the first path that reaches it, and reads at
most 64 KiB of its `SKILL.md`, only once it resolves to a regular file (never a pipe or device); a
`SKILL.md` that resolves into the worktree's `.git` or `.orchester` (*… leads into the worktree's .git or
.orchester*) or is not a regular file (*… is not a regular file*) refuses its start too. The search
`list_skills` uses while the setting is on is unchanged: it leaves such skills out and never refuses.

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
command, *The person reviewing this run denied <server> <tool>. Continue without it.* for an MCP call; when
automation denies through Orchester's MCP server, which names no person, *Denied by automation through Orchester's
MCP server. Continue without it.* and *Automation denied <server> <tool> through Orchester's MCP server. Continue
without it.*) and no
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
`DecidedBy::Permissions` and marked *allowed by the profile · <category>*; the run's Summary counts it as
*Allowed by the profile*, and its inline card says so): listed in the step's allowed commands and naming nothing outside the worktree, or a known-safe
read inside the worktree (`ls`, `cat`, `head`, `tail`, `wc`, `grep` without `-R`, `rg`, `sort`, `uniq`, `find`
without `-exec` or `-delete`, `sed` whose script only prints, selects or changes lines without `-i`, `awk` whose
program uses only the names an allow-list holds, the git subcommands that only read). In a step that edits
files (user decision 2026-10-05), a part that only writes files in the worktree or its writable folders runs too,
as the edit tools write there without asking (`CommandPart::edits_worktree`): `sed -i`, a redirect into a file
(`>`, `>>`, `>&file`), a here-document into one (`cat > f <<'EOF'`; a bare delimiter's body with `$` or a
backtick is not read), `tee`, `mkdir`, `touch`, `cp` and `mv` of files within (not of a folder or a link, nor
with `-r`, `-a`, `-P` or `-d`), `rm` of what lies inside. A write under what an earlier part of the line made asks
unless every folder down to it is there as a plain folder; on Windows a path rooted without a drive is not placed. Each target is
resolved with its links followed and refused where the native agent's file tools refuse a write
(`native::scope::protected`: any `.git` entry, `.orchester/` at the top of the worktree), and so is removing the
worktree or a writable folder itself; a read outside (`cp ~/x src/`) follows the rules for reads outside. In a
read-only step every write asks. In such a step a script file of the worktree run by an interpreter
(`python x.py`, `node`, `bash`, `pwsh -NoProfile -File`, the interpreter named bare; no inline code, module or
code loaded from elsewhere) runs too (decision 2026-10-06), unless a part before it in the line may change files, and
so does git in a repository nested in the worktree (`git -C <fixture>`, with only `-c user.name`/`user.email`, no
signing); the step's writable folders do not count for either. An allowed `cargo fmt` also allows `cargo +nightly fmt`. The classifier reads such
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
