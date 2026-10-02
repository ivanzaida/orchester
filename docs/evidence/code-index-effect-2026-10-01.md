# Code index effect on a real Codex task (index on vs off)

Date: 2026-10-01. This is the measurement that the ORC-38 entry in `docs/plan.md` lists as open ("before/after
measurement on a real task"). The user's stable Orchester (installed build of master `bd59eda`) was driven only
through its embedded MCP server. The numbers come from the project database, read in read-only mode
(`.orchester/orchester.db`). Scratch scripts and logs are in `.tmp/code-index-measure/`.

## Setup

- **Project settings › Code index** before the runs: *Use code index* was on ("On · agents search the index"), with
  "No index yet — built when the next run starts" and no index kept. The ripgrep row read "Found · ripgrep 15.1.0",
  "On PATH · agents get a copy in .orchester/tools". After run A the tab listed one index: `bd59eda`, "master ·
  ORC-46 · Run 14", Ready, "1.8 MiB · 1,164 files · 13,295 symbols · built today 00:27 in 0 s".
- **Project settings › General:** *Repository agent files* was off. Codex did not load the repository's
  `AGENTS.md`/skills itself. The project's published instructions (an imported `AGENTS.md` and one other) and the
  profile's skills reached the agent through Orchester in both runs.
- **Pipeline:** a new pipeline, *Measure · Implement only* v1, published. It has one step, *Implement*, with the same
  profile as step 1 of *Orchester change* (Implement, published v6, Codex, `gpt-6.1-sol`). Repository access is
  *Can write*. There are no instructions, no step budget, no approval gate and no tool approvals. Its step JSON is
  identical to step 1 of *Orchester change* v1.
- **Task text:** ORC-32's title, brief and four acceptance criteria, copied without changes. The brief says clearly
  what is wrong, so nothing was added. Both tasks had a run budget of 3,000,000 tokens and a 40-minute time limit.
- **Order:** run A (index on) ran to the end first. Then the setting was switched off and run B was started. The
  setting was switched back on after B finished. Master was `bd59eda982b1` for both runs. Both runs were based on
  it, and neither run needed an approval.
- Codex CLI on the machine: 0.159.2. Permissions for both launches: edit files plus `.orchester/target`, any
  command in the worktree, network on. The project environment sets `CARGO_TARGET_DIR` to the shared
  `.orchester/target`.

## Results

| | Run A — index on | Run B — index off |
|---|---|---|
| Task / run | ORC-46 · run #14 `01a0f437-0335-7214-b66b-6a9ee0878b7e` | ORC-47 · run #15 `01a0f44e-cfca-71ab-bd78-9d825b66f2f1` |
| Implement invocation | `01a0f437-090b-728e-abae-feda409feafa` | `01a0f44e-d3b7-747e-a03e-8b92e8e0c0a7` |
| Base revision | `bd59eda982b1` | `bd59eda982b1` |
| Code index (`invocation_code_index`) | given · revision `bd59eda` | off (project setting) |
| Outcome | succeeded · agent reported `done` | succeeded · agent reported `done` |
| Input tokens (total) | 3,750,167 | 1,975,510 |
| Cached input | 3,625,984 | 1,784,192 |
| Uncached input | 124,183 | 191,318 |
| Output tokens | 12,826 | 8,813 |
| Orchester "spent" (input − cached + output) | **137,009** | **200,131** |
| Before first edit (uncached / cached / output) | ≤ 124,183 / ≤ 3,625,984 / ≤ 12,826 (`at_most`) | ≤ 191,318 / ≤ 1,784,192 / ≤ 8,813 (`at_most`) |
| First edit: Codex `file_change` started / reported | 210 s / 275 s | 153 s / 158 s |
| First edit path | `crates/core/src/run/invocation.rs` (same patch: `summary.rs`, `ui/active_time.rs`, `ui/mod.rs`) | `crates/desktop/src/ui/active_time.rs` (same patch: `ui/mod.rs`) |
| Wall time / active time | 956 s / 956 s (no waits) | 570 s / 570 s (no waits) |
| Agent turns | 1 Codex `exec` turn · 10 messages | 1 Codex `exec` turn · 6 messages |
| Shell commands (all / before first edit) | 37 / 12 | 26 / 10 |
| Read/search sub-commands before first edit | 66 (7 of them skill files) | 40 (4 of them skill files) |
| Command output before first edit | 228,838 chars | 183,589 chars |
| Codex `file_change` items | 1 (later edits went through PowerShell string replacement) | 6 |
| Index files used | the index skill + `map.md` (one command, 21 s), then one `rg` over `symbols.tsv` (37 s). `tree.txt` and `outline/` were not used. | none (the index was not offered) |
| Branch diff vs base | 11 files, +364 / −11, commit `064c791` | 10 files, +306 / −8, commit `9979893` |

"Before first edit" equals the whole invocation in both runs. Codex `exec` reports usage once, when the whole turn
ends, and each run had only one turn. The count is therefore an upper bound (`at_most`) and does not tell the runs
apart. The time and the number of commands before the first edit are the useful early signals here.

Read/search sub-commands are the `;`/newline-separated parts of each PowerShell command that start with `rg`,
`Get-Content`, `Select-String`, `Get-ChildItem`, `git grep/show/log/diff/status`, `Test-Path` and similar. Codex
combines many reads into one command, so this number shows the volume of reading better than the command count
does.

## Observations

- **The index was offered and used, but only as an entry point.** Run A read the `orchester-codebase-index` skill and
  `map.md` in its first command. Its third command ran one `rg` over `symbols.tsv` for `active.*time|active_agent|form_keyboard`.
  That returned 40 rows, mostly `form_keyboard` test constants, which is noise. After that the agent went back to `rg`
  and `Get-Content` on the source, just like run B. It did not open `tree.txt` or any outline file.
- **Run A was not cheaper or faster on this task.** A spent 137k tokens against B's 200k, but its total input was
  1.9× larger (3.75M against 1.98M). Nearly all of that difference was cached input. It had 37 commands against 26,
  66 read sub-commands before the first edit against 40, took 210 s against 153 s to start the first edit, and 956 s
  against 570 s overall. The lower "spent" figure comes from a better cache hit rate (96.7% against 90.3%), not from
  less reading. It is not a saving that can be credited to the index.
- **Both runs produced nearly the same, sensible change.** Each adds a `ui/active_time.rs` component that ticks once a
  second through a lurq `Interval`, and only while an invocation is active and not waiting. Each takes an injected
  `ActiveTimeClock` for fake-clock tests, adds `display::active_duration` so that seconds stay visible after a minute
  (new key `time.minutesSeconds`), and applies it to *This run* and to the Ready-for-review run details. Each adds
  `crates/desktop/tests/form_keyboard/active_time.rs` and a `docs/plan.md` entry. Both agents reported that
  `tools/check.ps1` and their filtered tests passed.
  - A also caps the live value at the run's time limit and freezes it at `paused_at`/`finished_at`. This matches
    criterion 2 ("never runs ahead of the time limit logic") more directly. It adds `Invocation::is_active` in core.
  - B does not cap at the limit, and its props have public fields.
  - Neither run was design-verified, because the pipeline has no Verify step. Neither branch was merged.
- Both agents spent their first 1–2 minutes on the same things: reading skills (A read `lurq-ui` and
  `rust-code-quality` twice), the lurq 0.30.4 sources in the cargo registry (timers, `request_redraw_at`), and the
  task screen modules. The index has nothing on the lurq registry sources, which were a large part of the reading in
  both runs.

## Caveats

- **Single sample per arm.** With nondeterministic model output, one pair of runs cannot separate the index's effect
  from run-to-run variance. In this pair the variance, for example A re-reading skills and B landing a tighter first
  patch, is at least as large as any plausible index effect.
- **Order and shared state.** A ran first. The two runs shared `CARGO_TARGET_DIR` (`.orchester/target`), so B's cargo
  builds may have been faster on a warmer cache. A's first cargo command took 274 s; B's first `cargo check` took
  63 s. This affects wall time only, not tokens before the first edit.
- **Usage granularity.** Codex `exec` reports usage per whole turn. Both runs had one turn, so "tokens before first
  edit" is the whole run in both (≤). A per-request or per-item usage stream would be needed to measure what the
  index saves before the first edit.
- **First-edit detection.** Only Codex `file_change` items count. Edits made through PowerShell string replacement
  (common in both runs) are not seen, but that only matters after the first patch here.
- **Task shape.** This task is about UI timing and depends mostly on lurq internals outside the repository, which the
  index does not cover. A task that needs finding code across this repository would be a better test of the index.

## Conclusion

On this task and sample, the code index did not measurably help Codex. The agent consulted it once (`map.md` and one
`symbols.tsv` query), got noisy symbol hits, and then explored the source with `rg`/`Get-Content` as it did without
the index. Run A read more, took longer to its first edit (210 s against 153 s) and overall (956 s against 570 s), and
used 1.9× the total input. Its lower "spent" came from caching. Both runs produced an equivalent, reasonable
implementation.

A useful verdict needs:

- several runs per arm, alternating the order;
- separate cargo target directories;
- a repository-navigation-heavy task;
- ideally, per-request usage for Codex.

It would also help to look at why the symbol query was noisy. The agent searched a whole-line regex over
`symbols.tsv`, including the path column.
