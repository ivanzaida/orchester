# Agents ask: live turns and App Server isolation, 2026-10-01

Branch `claude/ask-outside`, after the isolation fix (`e7fe105`) and "agents ask" (`ec42eef`), with master
merged. One live turn per CLI ran through the runner (scheduler, store, real CLI, real approval path) on scratch
repositories under `.tmp/ask-outside-e2e/tmp/`, from a scratch `#[ignore]` runner test that was not committed
(`.tmp/ask-outside-e2e/live_ask_e2e.rs`). The test approved every request once through
`Scheduler::decide_tool_approval`, as the "Needs you" modal does. Neither the stable install nor the real
`.orchester/` data was used. Raw output: `.tmp/ask-outside-e2e/out/{codex,claude,claude-run1}/`.

## Codex CLI 0.159.2 (gpt-6.1-sol, effort low)

Connection: `C:\nvm4w\nodejs\codex.cmd` (npm launcher, started through its native program). Profile: edit files,
*Any command in the worktree*, network off (stays on on Windows). Recorded launch permissions:
`commands: AnyInWorktree`, `outside_commands: Some(SandboxApproval)`.

The task asked for two commands writing to `.tmp/ask-outside-e2e/outside/`, outside the writable roots:

| Command | What happened |
| --- | --- |
| `cmake -E touch …\outside\rule-marker.txt` | Ran **without asking** and created the file outside the writable roots. The user's `~/.codex/rules/default.rules` has `prefix_rule(pattern=["cmake"], decision="allow")`: an execpolicy `allow` rule runs the command outside the sandbox. App Server loads those rules and has no setting to leave them out; the enforcement panel now says so. |
| `Add-Content -Path '…\outside\escalated.txt' -Value 'escalated'` | First run inside the sandbox: denied (`Add-Content` error). The model then requested escalation; Orchester recorded one command approval: action `pwsh.exe -Command "Add-Content …"`, note "The sandbox denied this outside-workspace write. May I run this exact command once to append 'escalated', as requested?", target the worktree. Approved once, it ran once: `escalated.txt` holds one line. |

The step ended `question`, because the task's default acceptance criterion (README.md) conflicted with "change
no other file". The turn itself completed.

**Usage.** Orchester recorded `input 63869, cached_input 46976, output 416` for the invocation. The session's own
rollout (`~/.codex/sessions/2026/10/01/rollout-…-01a0f826-846c-73a3-b36f-a2b1e94e32f5.jsonl`, read only) ends with
`total_token_usage` `input 63869, cached 46976, output 416`: identical. The turn had four model responses (`last`
16218/15872/185 for the final one) and three `token_count` events without usage info (rate limits). The
previous code kept only the last `last` and would have recorded 16218/15872/185.

**Isolation.** Before the thread started, `config/read` listed the user's eight MCP servers (`ida-pro-mcp`,
`node_repl`, `openaiDeveloperDocs`, `pencil`, `plane`, `pw_emulator`, `pw_studio`, `x64dbg`). The runner turned each
off for the thread, and the step started, so its check that every server the step does not choose is `disabled`
passed. Probes without a model turn (`.tmp/ask-outside-e2e/probe_*.py`) showed:
- `-c mcp_servers={}` leaves all eight enabled, because tables merge;
- `notify=[]` replaces the user's `notify`;
- `features.plugins=false` removes the plugin servers `code-review` and `cua_repl`; with `features.apps=false`,
  `codex_app` is gone too;
- a `.codex/config.toml` in a folder of the trusted Orchester repository is loaded as a project layer, and its
  server is turned off the same way.

## Claude Code 2.1.286 (claude-opus-5-5, effort low)

Connection: `C:\nvm4w\nodejs\claude.cmd` (npm launcher, `@anthropic-ai/claude-code/bin/claude.exe` 2.1.286).
Profile: edit files, allowed commands `git`, network on. Recorded launch permissions: `commands: Allowed(["git"])`,
`outside_commands: Some(PermissionPrompt)`. Run 2 asked for three commands:

| Command | What happened |
| --- | --- |
| `git status --short` | Listed: ran without asking. |
| `node -e "console.log(6*7)"` | Unlisted: one approval (note "Print 6*7 with node"); approved once, printed `42`. |
| `echo $USERNAME` | Contains `$VAR`: one approval (note "Print current username"); approved once, printed `lurkm`. |

The run succeeded with the structured result. Usage recorded: `input 42551, cached_input 38550, output 663`.
In run 1, `node --version` in place of the `node -e` command ran **without asking**: Claude Code classifies version
queries as read-only, which the allowed-commands option says it runs. `echo $USERNAME` asked and was approved once
there too.

## Repository `.codex` settings (no model turn)

After the user's decision to keep their own Codex login, configuration and `~/.codex/rules`, only the
repository's `.codex` folder is left out. Probe: a scratch repository (`.tmp/codex-untrusted/repo`) with an
Orchester-style worktree (`.orchester/worktrees/w1`) holding `.codex/config.toml` (server
`orch_project_probe`, a model), `.codex/rules/x.rules` (`cmake` allow), `.codex/hooks.json` and
`.codex/skills/orch-proj-skill`. `probe.py` reads `config/read` with layers, `hooks/list`, an ephemeral
`thread/start` without a turn (with `RUST_LOG` for the execpolicy load lines) and its MCP inventory, under
scratch `CODEX_HOME`s (`cases.py`), with and without
`-c projects={"<worktree>"={trust_level="untrusted"},…}`:

| The scratch home trusts | Without the override | With it |
| --- | --- | --- |
| the repository (`h:\…\repo` or `H:\…\repo`) | server, hook, project model and `loaded 1 .rules files in …\w1\.codex\rules` | project layer disabled (*…\w1 is marked as untrusted*), nothing of it loads |
| the worktree | the same: loaded | disabled, nothing loads |
| the repository written with `/` | not trusted | disabled |
| a parent folder, or the drive | not trusted (no trust by ancestor) | disabled |

Only the worktree key matters once it is untrusted: marking just the worktree, in either drive-letter case,
disabled the layer of a trusted repository; a key with `/` or the `\\?\` prefix did not. Under the user's
real configuration, which trusts `h:\projects\orchester`, the nested scratch repository was untrusted (the
user's own rules still loaded from `~/.codex/rules`). Skills of `.codex/skills` load even untrusted
(`skills/list`) and are disabled by path, which Orchester does while *Repository agent files* is off. The
runner checks the layers before each thread (a loaded project layer refuses the step) and notes a disabled
one in the activity.

App Server finds a session `codex exec` recorded: `thread/read` (metadata only) of an exec-era thread of an
earlier Orchester run returned it with `source: exec`; the rollout file was unchanged. A thread id Codex has
no rollout for is refused with `no rollout found for thread id …`.

## Not covered

- No decline was made live; declines are covered by the runner tests with the fake CLIs.
- No Codex resume (`thread/resume`) ran live.
- No macOS run.
