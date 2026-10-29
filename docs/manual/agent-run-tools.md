# Tools agents get inside runs

Every agent step of a run (Claude Code or Codex) gets a set of Orchester tools, served by the project's **runner**
through a small relay process that the agent's CLI starts as an MCP server named **`Orchester`** (capital O, so no
project MCP server can take the name). Claude Code sees them as `mcp__Orchester__<tool>`; Codex sees bare names.

These tools are separate from the app's MCP server ([mcp-server.md](mcp-server.md)), which agents in runs never
reach.

## How they are served

- One relay per agent invocation. It connects to a loopback port of the runner with the invocation's random token.
- **No tool takes a project or a run**: the relay belongs to one invocation, and every path is relative to the run's
  worktree.
- Schemas are strict (`additionalProperties: false`); optional fields may be omitted or `null`.
- Answers are cut to **60 KiB** and the invocation's secret values are removed from them.
- `tools/list` annotations: `readOnlyHint`, `destructiveHint` (only `stop_run_process`), `openWorldHint` (tools that
  reach outside the worktree).
- Codex's per-call MCP timeout is set to 24 h + 60 s so long checks are not cut.
- The step's prompt gets an *Orchester tools* paragraph naming the tools it may use.
- Errors are plain-text tool errors (no `code`), e.g. `there is no Orchester tool named …`,
  `` `<tool>` is not available in this step``, `invalid input for <tool>: …`.

## Which tools a step gets, and whether a call asks

| Access class | Tools | Writing step | Read-only step |
|---|---|---|---|
| Read-only | `read_file`, `list_files`, `search_file`, `find_references`, `get_task`; `find_symbol` only with the code index (*Use code index*); `list_skills`, `read_skill` only when the step has skills | runs | runs |
| Writes the worktree | `edit_file`, `create_file`, `delete_file`, `move_file` | runs | not offered |
| Records | `save_memory`, `report_progress` | runs | runs |
| Run processes | `list_run_processes`, `stop_run_process`, `call_run_app` | runs | runs |
| Outside | `search_external_file`, `read_external_file` | asks; runs without asking inside the run's sources and Rust's dependency sources (Cargo registry/git checkouts, toolchain library sources) | same |
| Outside | `run_check` (step has network, ≥ 1 check offered) | runs a check whose access rules approve no call; otherwise asks once per run or at every call, as the tool's access rules say | same |
| Outside | `start_qa_app` (step has network) | asks | not offered |

- `run_check` and `start_qa_app` are not offered to a step without network.
- A call that asks reaches a person like any request outside the step's permissions; a run rule (*Approve similar*)
  may answer it. **Auto-approval and the reviewer never answer `run_check` or `start_qa_app`.**
- *Approve for every run of this project* on a `start_qa_app` card keeps a project rule tied to the QA script's git
  blob (revoked in Project settings › Runs › Approvals › *Approved for every run*).

## Tool reference

### Reading the worktree

| Tool | Parameters | Notes |
|---|---|---|
| `read_file` | `filename` (req., relative to the worktree root); `start_line` ≥ 1 (default 1); `length` 1–10 000 (default 2000) | Numbered lines. A file must be read before `edit_file`. |
| `list_files` | `pattern` (glob relative to `path`, e.g. `**/*.rs`; null lists all); `path` (folder; null = root) | Respects `.gitignore`; sorted; at most 1000 entries. |
| `search_file` | `term` (req., ≤ 1000 chars, a literal on one line, case-insensitive when all lower case); `limit_files` 1–200 (default 20) | For a name, definitions from the code index first, then each matching file with its matching lines. |
| `find_symbol` | `name` (req., name or part, any case); `kind` (`struct`, `enum`, `trait`, `function`, `method`, `class`, `interface`, `type`, `module`, `macro`, `const`, `static`, `impl`); `include_tests` (default false); `include_consts` (default false) | From the code index of the run's base revision: `path:line`, kind, signature; at most 50 hits; hits in files changed since are marked. Only with the code index. |
| `find_references` | `name` (req., exactly as the code writes it) | Definitions from the index, then whole-word uses; says when the index is off or out of date. |

### Changing the worktree (writing steps only)

| Tool | Parameters | Notes |
|---|---|---|
| `edit_file` | `filename`, `start_line` (≥ 1), `end_line` (≥ 0; `start_line − 1` inserts before `start_line`), `expected` (the lines exactly as they are now, without numbers; empty for an insertion), `content` (the replacement; empty removes) — all required | Replaces only if the lines are exactly `expected`; otherwise nothing changes and the current lines are returned. Later line numbers shift. |
| `create_file` | `filename`, `content` (both req.) | Missing folders are made. Refused when the file exists. |
| `delete_file` | `filename` (req.) | Shows in the run's changes. |
| `move_file` | `from`, `to` (both req.) | No file may be at `to`; folders are made. |

### Outside the worktree

| Tool | Parameters | Notes |
|---|---|---|
| `search_external_file` | `base_directory` (req., absolute folder); `term` (req.) | E.g. dependency sources. Refused: credential locations, `.git` folders, any `.orchester` folder, `orchester.db`, runner-private folders. |
| `read_external_file` | `filename` (req., absolute path); `start_line`, `length` | Same deny list; at most 56 KiB of lines, each line cut to 16 KiB; paths ≤ 4096 chars. |
| `run_check` | `name` (req.): one of the checks offered to the step (the tool's description lists them) | Runs a **command tool** of the project's Tools library that the run pinned and whose access rules let this pipeline/step call it — fixed arguments, in the worktree, with the agent's environment and the tool's timeout (≤ 24 h). Returns the exit code and the **end** of the output. |

### Processes and QA apps

| Tool | Parameters | Notes |
|---|---|---|
| `list_run_processes` | — | Processes whose executable lies inside this run's worktree, started after the run was created: pid, name, relative path, start time. |
| `stop_run_process` | `pid` (req.) | Stops a process and its children inside the run's worktree without asking (asks to close, waits briefly, ends survivors). Executables outside the worktree, the runner and agent CLIs are refused. |
| `start_qa_app` | `scenario` (req., `^[a-z0-9][a-z0-9-]*$`, ≤ 64 chars: a seed scenario of `tools/qa-app.ps1`, e.g. `tasks`, `task-states`, `decisions`); `fake_agents` (bool); `no_runner` (bool; not with `fake_agents`) | Builds and starts a **QA copy** of the app from the worktree (`tools/qa-app.ps1 -Name run-<number>-<step>`), off-screen and without focus. Asks a person first; the approval names the script's git blob and changed build-relevant files (`build.rs`, `Cargo.toml`, `Cargo.lock`, `.cargo/config*`, `rust-toolchain*`). Takes one of the project's **QA slots** (*QA apps at once*, 1–16, default 2) and waits while all are held. Starting again in the same turn replaces the app. The runner stops the app (and everything it started) and frees the slot when the turn ends, fails, asks, pauses or is stopped. 30-minute start limit. |
| `call_run_app` | `pid` (null = the QA app `start_qa_app` started in this step); `tool` (req., e.g. `lurq_windows`, `navigate`, `lurq_screenshot`); `arguments` (req. object, `{}` for none) | Calls one MCP tool of an app started inside this run's worktree, without asking. The token stays inside the runner; screenshots are saved in the run's own storage, `.orchester/runs/<run>/screenshots/`, and returned as absolute paths that every step of the run can read. |

### Task, skills, memory and progress

| Tool | Parameters | Notes |
|---|---|---|
| `get_task` | — | The run's task as the run started it (title, brief, acceptance criteria), the plans approved in this run, and the person's decisions and notes (requested changes, answers, messages). Markdown. |
| `list_skills` | — | The step's skills (Orchester's and the repository's) with source, description and reference files. |
| `read_skill` | `name` (req.); `file` (reference file relative to the skill folder; null = `SKILL.md`); `offset` ≥ 1; `limit` 1–10 000 (default 2000) | Skills are read by name, never by path. |
| `save_memory` | `kind` (req.): `fact`, `decision`, `observation`, `artifact`, `handoff`, `todo`, `warning`, `summary`; `text` (req., a few sentences, never a secret); `key` (≤ 120 chars; the same key replaces); `tags` (one word each, ≤ 40 chars); `importance` `low` \| `normal` (default) \| `high`; `scope` `task` (default) \| `project` | Active at once or waits for a person's approval, as the project's memory settings say. |
| `report_progress` | `text` (req., one line, ≤ 200 chars) | Replaces the step's progress line in the run's summary. |

### Claude Code only

| Tool | Purpose |
|---|---|
| `approve` | Claude Code's `--permission-prompt-tool`: input `{tool_name, input, tool_use_id}`; answers allow or deny with a message. Hidden from the model by Claude Code itself. |
| `ask_question` | Asks the person a question with **at least two** suggested answers: `question`, `header`, `options` (≤ 26 of `{label, description}`, distinct labels), `multi_select`. The call waits in the agent's turn (agent time stops counting) and returns the person's answer. A question without options ends the turn with the *question* outcome instead. Codex uses its own `requestUserInput`. |
