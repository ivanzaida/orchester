# Code index on vs off with Claude Code and Orchester's file tools (ORC-69)

Date: 2026-10-07. This is the re-measurement that T5 in `docs/plan.md` and ORC-69 ask for: what the code index gives
an agent now that it reaches the index through Orchester's file tools (`find_symbol`; `search` and `list_files`
answer from the index when it is on), not through raw `rg` over the TSV files. It follows
`docs/evidence/code-index-effect-2026-10-01.md`. Everything below comes from recorded run data in the project
database `.orchester/orchester.db`, opened read-only. No run was started for this file. The exact SQL is in
`.tmp/orc69/queries.sql`; `.tmp/orc69/run.py` runs it.

## Method

- **Pipeline:** *Measure · Implement only · Claude* v1 (pipeline version `01a1100b-f6e3-77cc-…`). It has one step,
  *Implement*, with repository access *Can write*, no step budget and no approval gate. The agent is Claude Code with
  `claude-opus-5-5`. All four runs used the same pipeline version, profile and instructions.
- **Tasks:** two source tasks, each copied twice: once run with *Use code index* on and once with it off.
  - ORC-49, *Count first edits made through the shell for Codex…*. This is a core/runner change and needs a lot of
    repository navigation.
  - ORC-54, *Activity: show Claude Code's permission_denied events as a readable row…*. This is a desktop/transcript
    change.
- **Arm check:** `invocation_code_index` records `given · 8fdf9559` for both ON runs and `off` for both OFF runs.
  So each arm is what its title says.
- **Tokens:** these are the `invocations` totals. *Spent* is input − cached + output, as Orchester counts it, and it
  equals `runs.used_tokens`. Every run had exactly one invocation.
- **Active agent time:** ended − started − inactive, as in `Invocation::active_millis`. No run waited for a person,
  and `inactive_ms` is 0 everywhere.
- **Tool calls:** the `agent_events` of kind `command_started`, classified by tool name, plus `file_changed`, which
  is how Claude Code's own Edit and Write appear.
  - *Reads:* `read_file`, `read_outside_file`, `Read`.
  - *Searches:* `search`, `list_files`, `find_symbol`, `Grep`, `Glob`.
  - *Edits:* `edit_file`, `write_file`, Edit and Write.
  - *Shell:* any other command, such as cargo, git, powershell or python.
  - *Other:* `git_status`, `git_diff`, `StructuredOutput` and `WebSearch`.
- **First edit:** the first edit tool call on a repository file. Edits under `.tmp/` are scratch scripts and do not
  count. I recomputed this myself; I did not take Orchester's `invocation_edit_progress`, for the reason given
  under *Measurement defect* below.
- **Tokens before the first edit:** the sum of the `turn_completed` usage of the turns recorded before the first
  edit.
  - The per-turn usage adds up exactly to the invocation totals.
  - For the edit Orchester did track in ORC-77, the method reproduces Orchester's own numbers exactly: 773,540 input,
    689,540 cached and 169 output.
- **Approvals:** the rows in `tool_approvals`.
- **Success:** the run status, the step status and the agent's reported outcome. The diff size is
  `git diff --shortstat base..checkpoint`.

## Runs

| Source | Arm | Task | Run | Run id | Base | Checkpoint |
|---|---|---|---|---|---|---|
| ORC-49 | on | ORC-76 | #40 | `01a115fa-ad6a-740b-9e3a-5700461a59a5` | `8fdf9559` | `4c1c8edc` |
| ORC-49 | off | ORC-81 | #45 | `01a11614-d179-77f4-a3e7-d233a1dcbf94` | `8fdf9559` | `9fe8a576` |
| ORC-54 | on | ORC-77 | #41 | `01a115fa-b7f6-71a7-a09d-3f10ce86ccbe` | `8fdf9559` | `4d0c7543` |
| ORC-54 | off | ORC-82 | #46 | `01a1168e-e9b6-7530-a604-59574723d1aa` | `31bf775b` | `ff8f25ca` |

**Earlier pair, ORC-46 and ORC-47.** These two can be assigned to arms honestly:

- ORC-46 is *Measure A*: run #14, `invocation_code_index` = `given · bd59eda`, so index on.
- ORC-47 is *Measure B*: run #15, `off`.

Both measured ORC-32, but they used a different pipeline (*Measure · Implement only*, Codex `gpt-6.1-sol`) and the
format-5 index from before ORC-48. That index was read with `rg` on the TSV files, because the file tools did not
exist yet. So they are listed for reference and are **not pooled** with the Claude runs. Their tool counts are in
the 2026-10-01 file.

## Results

| | ORC-49 on (#40) | ORC-49 off (#45) | ORC-54 on (#41) | ORC-54 off (#46) | **Mean on** | **Mean off** |
|---|---|---|---|---|---|---|
| Input tokens | 16,478,237 | 13,539,307 | 14,294,571 | 9,360,996 | 15,386,404 | 11,450,152 |
| Cached input | 16,222,309 | 13,309,117 | 14,033,448 | 9,142,110 | 15,127,879 | 11,225,614 |
| Output | 79,038 | 70,387 | 63,972 | 52,278 | 71,505 | 61,333 |
| **Spent** (in − cached + out) | **334,966** | **300,577** | **325,095** | **271,164** | **330,031** | **285,871** |
| Active agent time | 1,618 s | 1,309 s | 1,579 s | 1,358 s | 1,598 s | 1,334 s |
| Time to first repository edit | 338 s | 299 s | 291 s¹ | 308 s | 315 s | 304 s |
| Spent before first edit | 136,839 | 152,174 | 162,048 | 141,396 | 149,444 | 146,785 |
| Turns before first edit | 25 | 29 | 34 | 25 | 29.5 | 27 |
| Tool calls (all) | 128 | 117 | 106 | 93 | 117 | 105 |
| — reads | 42 | 35 | 32 | 28 | 37 | 31.5 |
| — searches | 24 | 26 | 23 | 22 | 23.5 | 24 |
| — edits | 29 | 26 | 21 | 22 | 25 | 24 |
| — shell commands | 31 | 27 | 27 | 17 | 29 | 22 |
| — other | 2 | 3 | 3 | 4 | 2.5 | 3.5 |
| Tool calls before first edit | 38 | 37 | 45 | 42 | 41.5 | 39.5 |
| Distinct files read before first edit | 22 | 19 | 18 | 18 | 20 | 18.5 |
| `find_symbol` calls | 1 | — (not offered) | 1 | — (not offered) | 1 | — |
| `search` + `list_files` calls (index-backed when on) | 23 | 26 | 19 | 20 | 21 | 23 |
| Index files read (`map.md`, `symbols*.tsv`, skill) | 0 | — | 0 | — | 0 | — |
| Approvals requested (all decided automatically) | 7 | 3 | 6 | 6 | 6.5 | 4.5 |
| Outcome | succeeded | succeeded | succeeded | succeeded | 2/2 | 2/2 |
| Diff vs base | 20 files +821/−198 | 22 files +682/−177 | 15 files +721/−168 | 12 files +420/−36 | | |

¹ ORC-77 first wrote a scratch script, `.tmp/orc54/dump.py`, at 57 s. Its first repository edit came at 291 s.

All approvals were decided by the auto-mode reviewer or by the permissions. None was decided by a person, and none
was denied. ORC-82 also had 3 refusals by Claude Code itself (`permission_denied`).

For reference, the earlier Codex pair on ORC-32:

| | Index on (ORC-46, #14) | Index off (ORC-47, #15) |
|---|---|---|
| Spent | 137,009 | 200,131 |
| Active time | 956 s | 570 s |
| First edit | 275 s | 158 s |

## Observations

- **The agents barely touched the index.**
  - Each ON run called `find_symbol` once. In #40 it returned one struct, `CommandScope`. In #41 it returned two
    test helpers, `native_run`. Neither call changed what the agent read next.
  - No ON run opened `map.md`, a symbols file or the codebase-index skill.
  - Searches (`search` + `list_files`) were about as frequent in both arms: 21 against 23 on average.
  - Reading was the same in both arms: about 20 distinct files before the first edit, and 25–34 turns before it.
  - The prompt's map excerpt (at most 8 KiB) cannot be seen in the data: `invocation_prompts` stores only a
    reference.
- **The ON arm was more expensive in both pairs.**
  - Spent tokens were higher with the index on: +11% for ORC-49 and +20% for ORC-54, +15% on average.
  - Active time was also higher: +24% and +16%.
  - The ON runs also wrote larger diffs: +821/+721 lines against +682/+420. So part of the difference is more work,
    not more searching.
- **The early signals point in opposite directions.** Before the first edit, ON was cheaper and slower for ORC-49
  (137k against 152k spent; 338 s against 299 s). For ORC-54 it was dearer and faster (162k against 141k; 291 s
  against 308 s). Both gaps are about 10–15%, which is the size of run-to-run noise.
- **Every run succeeded** and passed its step. No run was reviewed or design-verified (the pipeline has only
  Implement), so quality is not compared.
- **The earlier Codex pair does not change the picture.** That index-on run spent less (137k against 200k) only
  through a better cache hit rate. It read more and took longer.

## Confounds and noise

- **Sample size:** n = 2 per arm for the comparable Claude runs. The model is nondeterministic, so differences of
  10–20% between two runs are within what repeated runs of one task produce. No difference here is outside that.
- **Order was not alternated:** both ON runs ran first. The OFF runs ran later.
- **Machine load differed.**
  - The two ON runs ran at the same time, and each overlapped three other agents for almost its whole duration
    (runs #40–#43).
  - ORC-81 overlapped two or three agents, and ORC-82 overlapped one.
  - Cargo builds and checks therefore ran under heavier load in the ON arm. That inflates its active time and time to
    first edit.
- **Different base for ORC-82:** `31bf775b` instead of `8fdf9559`. It adds only the five ORC-80 agent-slot commits,
  which are unrelated to ORC-54. Neither ON result was merged into that base.
- **Index-backed `search` and `list_files` cannot be separated:** their answers do not say whether they came from the
  index. Only `find_symbol` and index file reads are unambiguous index use.

## Measurement defect found

Orchester's first-edit tracking only counts `Edit`, `MultiEdit`, `Write` and `NotebookEdit`
(`EDIT_TOOLS` in `crates/core/src/connection/stream/claude.rs`). It does not count Orchester's own
`mcp__Orchester__edit_file` and `write_file`, which the prompt now steers Claude Code to use. The effect on these
runs:

- ORC-81 and ORC-82 recorded no first edit at all.
- ORC-76 recorded its first built-in Edit, a `Cargo.toml` change at 794 s. Its first real edit was at 338 s.
- ORC-77 recorded the scratch script.

The run details' *Tokens before first edit* is therefore wrong for Claude runs that use the file tools. This is a
separate fix and was not changed here.

## Verdict

**Does not help, as measured.** With Claude Code and Orchester's file tools, the code index made no measurable
difference:

- There was no consistent gain in tokens, time to first edit or files read.
- Total spend and active time were higher with the index on in both pairs, although the load and order confounds
  favour the OFF arm.
- The agents used the index once per run.

n = 2 cannot rule out a small effect. But the near-zero use of the index tools means there is little for an effect
to come from.

## Recommendation

*Use code index* is on by default today: a missing setting reads as on (`crates/store/src/project_db/code_index.rs`),
and this project has it on. The data does not justify keeping it on by default.

- **Make it default off**, or keep it on only because it costs little. The data shows no benefit, and the index adds
  a prompt part, an index build and a `find_symbol` tool the agents hardly call.
- **Before any re-measurement:**
  - fix first-edit tracking for `edit_file`/`write_file`;
  - run 3 or more pairs per task with the order alternated, one agent at a time;
  - log tool arguments or whether an answer came from the index, so index-backed `search`/`list_files` use can be
    counted;
  - make the prompt steer harder toward `find_symbol`.

  Measuring again without these changes would mostly measure noise.
