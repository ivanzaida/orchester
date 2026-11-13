---
name: work-outside-orchester
description: How Claude drives any Orchester module to the end outside Orchester's pipelines — Orchester's MCP (or a docs tracker when it is down) as the task tracker, at most five children at a time (Opus implementers; Sonnet reviewers, design verifiers and design agents), at most three of them building, in five pre-warmed build slots, small tasks verified in batches of four, a module branch, an acceptance gate that takes nothing untested and no unclear UX, and a per-module decisions file the person reviews later. Use when the person says to work a module, its tasks or a board "outside Orchester", to start, continue or resume such a module, or to orchestrate several tasks with child agents. Supersedes the module-specific parts of `team-module-orchestration`.
---

# Working a module outside Orchester

You (the main thread) orchestrate. You write no product code: you plan, hand out tasks, decide, merge, keep the
tracker and the logs. Children do the work.

The non-negotiables:

1. **Orchester's MCP is the tracker.** Tasks are read, created and moved there, and statuses follow the work.
2. **Nothing untested is accepted.** A report that could not test something is not done; you find out why, and when
   something is missing you make a task for it, drive it to the end, and come back.
3. **No unclear or bad UX is accepted**, whoever proposes it: a child, a brief, or the design itself.
4. **The person may be away.** You keep going, and every decision or deviation from the design you take yourself goes
   into the module's decisions file with screenshots, for the person to review.
5. **Never more than five children at a time, and never more than three of them building.** Implementers of normal
   tasks are Opus 5.5; reviewers, design verifiers, design agents, research and small tasks' implementers are Sonnet
   5.5; all with high reasoning ([Spawning a child](#spawning-a-child)).
6. **Builds start warm.** Children work in five pre-warmed slots whose `target/` is already built, through sccache,
   so a build compiles only what changed instead of taking twenty minutes.
7. **Small tasks skip the per-task review**: they are verified in batches of four, then the batch goes through
   the implement–review–verify loop as one.
8. **Finished workspaces are reset or removed.**
9. **You only orchestrate.** Other work the person hands you meanwhile (a handoff, a fix elsewhere) goes to its own
   session or its own child, never into this thread between reports: it slows both and competes for the machine.

## The module's files

Everything of yours lives in `.tmp/orchestrator/<module>/` (git-ignored; `<module>` is the module's slug, e.g.
`task-page-v2`):

| File | What |
|---|---|
| `pipeline.md` | The log: one line per event or decision, the person's rulings under `## Person (YYYY-MM-DD): …`. |
| `STATE.md` | The resume snapshot (see [Resuming](#resuming)). |
| `decisions.md` | For the person: decisions and deviations you took, and what needs them (see [Working without the person](#working-without-the-person)). |
| `shots/` | Screenshots `decisions.md` links (design and app, same state and size). |
| `briefs/ORC-<n>-<role>-<round>.md` | Every brief you sent, verbatim, so a replacement child gets the same. |
| `coverage.md` | The design-to-task map of [Starting a module](#starting-a-module). |
| `tracker-pending.md` | Tracker writes waiting while the MCP is down. |
| `briefs/_rules.md` | The rules every brief opens with ([The implementer's brief](#the-implementers-brief)), written once per module. |
| `shots/design/` | Every frame of the module's boards, exported once ([Design (Pencil)](#design-pencil)). |
| `scripts/` of this skill | `tokens.mjs` (a task's token totals) and `tracker.mjs` (the docs tracker's writer), run from here ([The tracker](#the-tracker)). |

Git: the module branch `module/<module>`, your merge checkout `.tmp/worktrees/claude-module-<module>`, the warm slots
`.tmp/worktrees/claude-slot-1` … `claude-slot-5`. Task branches `claude/orc-<n>-<short-name>`, batch branches
`claude/batch-<k>-<module>`.

## The tracker

The app's MCP server (`orchester` in this session's MCP list) is the only source of tasks and their state:
`list_modules`, `get_module`, `get_task`, `list_tasks`, `create_task`, `update_task`, `remove_worktree`
(`docs/manual/mcp-tools-work.md`).

**When the MCP is unavailable** (the app is closed, the connection failed): tell the person once that it failed and
that you continue. Meanwhile read from a snapshot, never the live file:

```powershell
python -I -c "import sqlite3; s=sqlite3.connect('file:H:/projects/orchester/.orchester/orchester.db?mode=ro',uri=True); d=sqlite3.connect('H:/projects/orchester/.tmp/orchestrator/dbcopy/orchester.db'); s.backup(d); d.close()"
```

The database is in WAL mode: copying the `.db` file alone gives stale or torn data, so always use the backup API.
Refresh the snapshot at every session start. Queue every write you would make in `tracker-pending.md` (tool,
arguments, why) and apply them in order once the MCP answers; until then the snapshot plus that file is the state.
Never open `.orchester/orchester.db` for writing.

**When the module has no tasks in Orchester** (the person asked for a docs tracker, or the MCP stays down for the
whole module): the tracker is `docs/plan/<module>/` on the module branch — `README.md` with one table row per task
(`| ID | Task | Kind | Depends on | Status | Branch | Tokens |`), the person's decisions and the design frames, and
one file per task (`<ID>-<name>.md` with `Status:`, `Branch:`, `Tokens:` lines, the goal, the design, *Done when*).
You keep it with `tracker.mjs` (reads a JSON list of `{id, status, branch, tokens, row}` and rewrites the row and the
task file's lines) and commit it on the module branch with each merge. Follow-ups a review leaves become numbered items
of a polish task or a new task file, never lost in a report.

**Tokens per task.** The person wants each task's cost: after its merge, sum the usage of every child that worked on
it (all rounds, reviews, verifications, merge rounds). `node .claude/skills/work-outside-orchester/scripts/tokens.mjs --skip <your session id> <brief file names…>` finds each child's session
(`~/.claude*/projects/<repo>/*.jsonl`) by the brief its first message names and adds up input, cache and output
tokens. Write both to the tracker: *total (out N)* — the total is mostly cache reads, the output is what was written.

**Statuses follow the work:**

| Moment | Local task | Team task |
|---|---|---|
| Ready to start (dependencies merged) | `todo`, `blocked_reason: null` | `todo` (Admin), `blocked_reason: null` |
| Waits on another task | `blocked_reason: "waits for ORC-<m>: <what>"` | the same |
| A child works on it | `in_progress` | stays `todo`: the server does not offer `in_progress`/`in_review` (`team_status_not_offered`); `STATE.md` holds it |
| Under review or verification; a small task implemented into its batch | `in_review` | as above |
| Merged into the module and accepted | `done` | `done` (claim holder or Admin) |
| Parked (needs the person, see below) | `blocked_reason: "needs the person: <what>"` | the same |
| Abandoned by the person's decision | `canceled` | `canceled` (Admin) |

A refusal (`not_admin`, `not_holder`, `decision_required`, …) is logged, goes into `decisions.md` under *Needs the
person*, and is never reported as done. Stale briefs are corrected with `update_task` (details of a team task are an
Admin's; a refusal goes the same way).

**Orchester must not work the same task.** Before a child takes a task, `get_task`: if it is queued or has a run that
has not ended, do not take it; log it and ask the person. Never `run_task` a module task.

**Workspaces of finished work are cleaned up** once a task (or batch) is merged or abandoned:

1. Stop the QA copies started for it, by their pids (on Windows a running copy locks files).
2. Copy the screenshots `decisions.md` needs into `shots/`; then delete `.tmp/qa/orc-<n>/`.
3. Reset its slot ([Warm slots](#warm-slots)): uncommitted changes in an abandoned task are first saved with
   `git -C <slot> diff > .tmp/orchestrator/<module>/patches/ORC-<n>.patch`.
4. Delete the merged task branch (`git branch -d`; never `-D` on unmerged work without logging the patch).
5. For each ended Orchester run of the task that kept a worktree: `remove_worktree`.
6. Any other checkout made for the task (a cold one, see below) is removed with `git worktree remove`.

## Starting a module

1. **Read the module**: `get_module`, then `get_task` for every task. Write each brief to `briefs/` as the tracker
   has it. Build the dependency graph from `blocked_by` and the briefs' `Depends on:` lines.
2. **Audit design coverage** before any work. For each design board the briefs name, list its frames (Pencil
   `Get(<board id>, …)` to depth 2, and the state cards inside rows such as rails) and map every frame to the task
   that builds it, and mark each task small or normal ([Small tasks](#small-tasks-batches-of-four)). Write the map to
  `coverage.md`. Every frame without a task gets one (`create_task` in the module,
   brief in the module's style, its dependencies set); every brief that names a frame the board no longer has, or
   misses one it should cover, is corrected.
3. **Check briefs against the code.** A brief written before later merges may delete or move code that now holds new
   behaviour (e.g. a brief removing a file that a newer feature lives in). Grep the files each brief names on
   `origin/master`; fix the brief so the newer behaviour is carried over, not dropped.
4. **The module branch.** If `module/<module>` exists, fetch it and merge `origin/master` into it. Otherwise create it
   from `origin/master` after `git fetch` — never from the main checkout, whose working tree holds the person's
   uncommitted edits — and push it (`git push -u origin module/<module>`). Create your merge checkout
   `git worktree add .tmp/worktrees/claude-module-<module> module/<module>`.
5. **Warm the slots** ([Warm slots](#warm-slots)) while you do the planning: they take the longest, and the first
   children wait for them.
6. **Plan waves**: tasks whose dependencies are met, ordered by how much they unblock; and the hot files (below).
   Write the plan to `STATE.md`, note in `pipeline.md`, tell the person in two lines.

## Scheduling

- **Never more than five children at once, and at most three building.** This is a hard cap, not a target to round
  up. A child builds when it runs cargo: implementers, reviewers and design verifiers do; design agents and research
  usually do not. More than three builds on this machine (16 cores, 32 threads) oversubscribe it: UI tests time out
  under the load and each timeout costs a re-run. Every role counts:
  implementer, reviewer, design verifier, batch verifier, design agent, research. Before every `delegate_task`,
  count the running children in `STATE.md` and with `task_status`. A child you cancelled counts until the cancel is
  confirmed. Children start no children of their own: every brief forbids the Agent tool, `delegate_task` and
  workflows. Refill a freed place at once. Finishing beats starting: reviews, verifications and fix rounds of
  finished work go before new implementations.
- **The build environment.** Every child that builds sets, for each cargo command, the environment Orchester's own
  runs use (the project's variables), so every checkout shares one compiler cache:

  ```powershell
  $env:RUSTC_WRAPPER = "C:\Users\lurkm\AppData\Local\Microsoft\WinGet\Packages\Mozilla.sccache_Microsoft.Winget.Source_8wekyb3d8bbwe\sccache-v0.18.0-x86_64-pc-windows-msvc\sccache.exe"
  $env:SCCACHE_DIR = "E:\sccache"
  $env:SCCACHE_CACHE_SIZE = "30G"
  $env:CARGO_BUILD_JOBS = "8"    # 32 threads: at most three building children and your own module checks
  ```

  Take the values from the project's variables if they changed (`get_project`, or the snapshot's
  `project_environment` table), not from this file. On macOS: `RUSTC_WRAPPER=sccache` with the Mac's own
  `SCCACHE_DIR`. Leave incremental compilation on. sccache caches the dependencies, which are most of the build, and
  incremental keeps the edit-and-rebuild loop on the workspace's own crates short inside a warm slot. `.cargo/config.toml`
  clears `RUSTC_WRAPPER` only for processes cargo starts (nested cargo in tests), not for the build itself. If sccache
  fails (`sccache --show-stats` errors, "Access is denied"), the child says so and builds without it rather than
  stopping; you record it in `pipeline.md`.
- **Heavy UI suites run one at a time.** A suite that drives windows and takes minutes (`--test team_qa`, the full
  `--test pipelines`, `--test agents` without a filter) runs under the machine-wide lock
  `.tmp/orchestrator/heavy-test.lock`: a child creates the file with its task and pid before the suite and deletes it
  after; while it exists, another waits. A test that times out is re-run alone before it counts as a failure.
- **Test tiers.** While it works, an implementer runs only the filters of what it changed. Before its report it runs
  the brief's suites once. A reviewer re-runs only what the report's claims rely on, plus the check. You run the
  module's heavy suites once, after the merge. Nobody runs the same suite twice on the same commit.
- **Disk**: before each new child and each warm-up, check free disk on `H:` and `E:` (`df -h`, never `du` on a
  `target/`: it takes minutes). Under 100 GB on either, clean up first: finished workspaces, then
  `target/debug/incremental` of idle slots. Do not wait for that: a slot's `target/` grows past 100 GB in a day of
  fix rounds, so delete `target/debug/incremental` of a slot every time its task merges and the slot goes idle.
  Deleting it under a running build only costs that build a retry; a full disk stops every child at once.
- **Hot files.** Before spawning, list the files each brief will touch (its paths, plus a grep of the code it
  names), and write an ownership line in each brief ("you own …; do not edit …"). Two live branches never edit the
  same file — not even "a few lines": the later task waits for the first to merge, or both go to one child in one
  checkout. A shared file that several tasks must touch (a screen's `render`) is first split by its own small task
  (one module per feature, e.g. `panel/versions.rs`) and merged, so each task then owns its own file. A locale file or
  a test binary's `main.rs` module list is the exception: both sides append, and you resolve those conflicts yourself.
- **The module moved.** After every merge, a live task branch that touches the merged task's area merges the module
  at its next round (the implementer does it, as the round's first step), not at the end: conflicts stay small.
- **Dependencies.** A task starts only when everything it depends on is merged into the module.
- **Master moves.** At every session start, and before merging a task that touches files master changed since the
  module last took it, merge `origin/master` into the module (see [Merging](#merging)).

## Warm slots

A fresh checkout has no `target/`, so its first build compiles the whole workspace, which takes about twenty
minutes. T3's `isolation: "worktree"` makes exactly that, and it puts the checkout under `C:\Users\<user>\.t3\worktrees`,
on another drive than the repository. So children do not get T3 checkouts. They get one of five warm slots that you
keep on the repository's drive. This departs from T3's default isolation on the person's instruction (2026-11-10).

- **Create** once (slots outlive modules): `git worktree add --detach .tmp/worktrees/claude-slot-<i>
  module/<module>`, for i = 1…5.
- **Warm up** a slot by building, in it, what the work builds: `tools/check.ps1` (clippy), `tools/qa-app.ps1` (the QA
  app, the seed, the runner) and the filtered tests. Use the build environment above:

  ```powershell
  cargo clippy --workspace --all-targets --all-features
  cargo build -p orchester-app --features perf_profile --example qa_app --example seed
  cargo build -p orchester-runner --features test-agent --bins
  cargo build -p orchester-runner --bin orchester-runner
  cargo test -p orchester-app --test agents --no-run   # and the other test binaries the module's briefs name
  ```

  Never `cargo test --workspace --no-run`: it links every test binary of the workspace, fills the disk (well over
  100 GB across five slots), and the module needs few of them.

  You run warm-ups yourself, with `run_in_background`, at most two at once. They are not children and do not count
  toward the five. Warm-ups go first at a module's start, and again for an idle slot after the module moved by several
  merges, so the next child starts close to the head. A warm-up that fails means the module head is red: stop and
  find out why before giving that slot out.
- **One slot per live task or batch**, from its implementer through review, verification and fix rounds. The
  reviewer and verifier of a task work in its slot, on the branch the implementer left checked out, and do not commit;
  cargo's lock queues their builds. Record every assignment in `STATE.md`. A parked task gives its slot back (its work
  is committed on its branch). Design agents and read-only research children need no slot.
- **Hand a slot over** to the next task:
  1. Check that `git -C <slot> status --short` is clean; otherwise save a patch (see the tracker's clean-up).
  2. Run `git clean -fd`. It leaves ignored files, so `target/` stays.
  3. Delete the slot's `.tmp/qa/` and `.tmp/check-tests/`.
  4. `git switch --detach module/<module>`.

  Git rewrites only the files that differ, and the rest keep their times, so cargo rebuilds only what changed.
- **Size**: a slot loses `target/debug/incremental` when its task merges (see Disk above); it stays warm enough,
  since sccache keeps the dependencies.
- **No warm slot free** (all five hold tasks, one is broken or being warmed): the next task waits. Never start a
  child in a cold checkout just to fill a place.

## Spawning a child

`delegate_task` with:

- `target: { providerInstanceId: <the Claude provider instance of the account the person runs now, from
  orchestrator_capabilities>, model: <below>, options: { effort: "high" } }` — never an id copied from an old log. The
  model: `claude-opus-5-5` for the implementer of a normal task (every round of it, and its merge rounds): a first
  pass that holds saves a whole review–verify–fix round of two to three hours. `claude-sonnet-5-5` for reviewers,
  design verifiers, design agents, research and small tasks' implementers;
- `role`: `implementation`, `review`, `design`, `test` or `research`;
- `isolation: "shared"`. The child starts in the main checkout and moves to its slot itself (below);
- `title`: `ORC-<n> · <role> · <round> · slot <i>`.

The brief names the slot's absolute path and opens with these rules. The child's checkout is that slot: it changes to
it first, keeps every command and every file it writes inside it, and never changes anything else under the
repository's main checkout. The only exception is writing screenshots to the absolute `shots/` path the brief gives.
Then its first git step:

- an implementer of a normal task: `git switch -c claude/orc-<n>-<name>` from the slot's detached module head. In a
  fix round the branch is already checked out;
- a small task's implementer: `git switch claude/batch-<k>-<module>`. The batch's first implementer creates it with
  `-c`;
- a reviewer or verifier: no switch; it checks that `git branch --show-current` is the task's branch, and stops
  otherwise.

Save the brief to `briefs/` before sending. A child sees nothing of this thread. End the turn after spawning:
completion wakes you; do not poll. `task_status` only for a child silent far longer than its task should take. Steer
a running child with `t3_thread_send` `mode: "steer"`. Every review or re-check is a new child. A fix round or a
merge round goes to the task's own implementer (`t3_thread_send` to its thread, with a new brief file for the round):
it knows its code.

**Briefs are templated.** `briefs/_rules.md` holds the rules below once; an implementer's brief is the task's tracker
text, your context (what is merged, file ownership, decisions, known failures) and those rules. A round-N brief names
the task's round-1 brief and the rules, and lists only what changed: the findings to fix, the commits since, and what
to re-check.

### The implementer's brief

1. The tracker's brief and criteria, verbatim, with the task's key.
2. The design: the absolute `file:///` URI of `crates/desktop-app/design/orchester.pen` in the MAIN checkout (first
   line of `git worktree list`), the board's id and the frame ids, and the instruction to verify the board by id in
   its first `execute` and stop if it is not there (names repeat: two boards can both be called `Task - v2`).
3. Decisions by the person and by you that apply ("follow them"), with `decisions.md` ids.
4. What is merged that it builds on; which live branches touch nearby code ("keep X, don't move Y").
5. The rules:
   - work only in its slot; never push; never touch master, the main checkout, the installed app or its data;
   - start no agents of its own (no Agent tool, `delegate_task` or workflows);
   - read and follow `AGENTS.md` and the skills `rust-code-quality`, `lurq-ui` (with references), `orchester-ui`;
   - commit with conventional messages ending in the Co-Authored-By line of its own model (e.g. `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`);
     never change `version` in `crates/desktop-app/Cargo.toml`;
   - the build environment of [Scheduling](#scheduling), set before every cargo command; every `cargo test` with
     `-p <crate> --test <binary> <filter>`;
   - the check: `powershell -File tools/check.ps1` (`bash tools/check.sh` on macOS); known failures on master are in
     `tools/check-tests-known-failures.txt`, so it reports only new ones;
   - its own screenshots only from a QA copy (`tools/qa-app.ps1 -Name orc-<n> …`), data under `.tmp/qa/orc-<n>/`;
     stops its own processes by pid;
   - **before its report it runs every suite the brief names, once, and reports each result**: a suite it skipped is
     a finding in review, and a red suite it did not run sends the task back (a skipped `--test pipelines` once cost
     a whole round);
   - **when a tool it needs is unavailable** (Pencil not connected, the app's MCP down), it uses the frames' exports
     in `shots/design/` and says so; it never guesses a size or a colour;
   - **the edge cases of its own feature**: for every action it adds, what happens with the neighbours (a step others
     return to on failure, a member of a parallel group, a running save, an open question) — tested, not assumed;
   - no design verification of its own work;
   - **it tests every state and behaviour it builds. If it cannot test something, it says so plainly with the exact
     reason and what would make it testable — never leaves it out, never replaces the test with a guess;**
   - **if the brief or the design asks for something a person would find unclear or surprising, it stops and
     reports the problem with a proposal instead of choosing silently.**
6. The report: SHAs; what was built per frame or step; how to open each state in a QA copy (scenario, steps); tests
   run with results; the check result; anything not done, not tested, or deviating from the design, each with why.
   If the task is too large, it finishes steps in order, keeps each commit green, and says where it stopped.

### The reviewer's brief

The tracker's brief and the range (`git log/diff module/<module>...claude/orc-<n>-<name>`), never your conclusion.
It edits nothing; re-runs the tests the claims rely on and the check; checks the version line, the hot-file rule,
AGENTS.md's hard requirements, and the UX gate below. It returns PASS or FAIL with findings ranked blocking, major or
minor, each with file:line and a concrete failure scenario, and lists anything it could not verify.

### Rounds after the first

The first round of a normal task has a reviewer and a design verifier, as below. A later round has **one** child that
does both, with one build and one QA copy, unless the round changed only what a person sees (then a design verifier
alone) or only logic with no visible change (then a reviewer alone). A merge round (the module merged into the task's
branch) needs no new review when both sides had passed and the implementer reports how each conflict was resolved
with its suites green; your checks after the merge are its gate.

### The design verifier's brief

`.claude/skills/design-verification/SKILL.md`, the URI and frame ids as above, how to reach each state (from the
implementer's report), and: save the Pencil and app screenshots of every compared state as
`<main checkout>/.tmp/orchestrator/<module>/shots/orc-<n>-<frame id>-{design,app}.png` (an absolute path, not the
slot's `.tmp`); report every state it could not reach, with
why; flag UX problems even where the app matches the design. It hovers and focuses every interactive element it verifies and compares
the bounds of what is beside it before and after: anything that moves is a major finding, design or not.

## Small tasks: batches of four

A task is **small** when it only changes what a person sees in place: a label or a word, a row moved, a spacing,
color, text or icon role, an order, a tooltip. It touches one screen and roughly 50 changed lines or fewer, and adds
no state, logic path, type, persistence, runner, IPC or MCP change. You classify each task at planning and mark it in
`coverage.md`. If an implementer finds a task is bigger than that, it stops and reports, and the task becomes a
normal one.

Small tasks get no per-task review or verification. They go through batches instead (**N = 4**):

1. **The open batch.** One batch collects at a time, in one slot, on the branch `claude/batch-<k>-<module>` from the
   module HEAD. Each small task is one implementer child with the normal brief and the same testing rules. Children
   work in the batch's slot one after another, never two at once, and each commits its task as its own commit
   `… (ORC-<n>)`. The hot-file rule applies between the batch
   and live normal tasks.
2. **Verification at four.** When the batch holds four implemented tasks, close it: start one **batch verifier**
   for all four, and open the next batch. The verifier follows the design verifier's brief for every frame of every
   task. It also re-runs the tests the implementers cite and the check, reads the batch diff against the UX gate and
   AGENTS.md's hard requirements, and returns one verdict per task.
3. **Fewer than four** may be verified when the batch would otherwise wait: no further small task is ready, slots
   sit idle waiting for one, or the module is ending.
4. **The loop on the batch.** If any task in the batch has findings, run the loop on the batch as a whole. One
   implementer fixes all the batch's findings on the batch branch, then a reviewer reviews the batch's whole range,
   then a verifier verifies all its tasks. Each is a new child that covers the whole batch. Repeat until every task
   passes, for at most two rounds. A task that still fails is taken out (its commits reverted on the batch branch)
   and becomes a normal task with its findings in the brief.
5. **Merge** the batch as one `--no-ff` merge, `Merge batch <k>: ORC-<a>, ORC-<b>, ORC-<c>, ORC-<d>`. Every task in
   it then becomes `done`, and its slot is cleaned up as for any task.

Everything else in this skill applies to small tasks too. Untested is not accepted, the UX gate applies, and
deviations go into `decisions.md`.

## Acceptance gate

A task (or a batch, for small tasks) is merged only when all of these hold:

- the reviewer's verdict is PASS, with no blocking or major finding open;
- the design verifier reports no blocking or major mismatch for every frame the task covers (any user-visible
  change);
- the check passes on the task branch after it contains the module HEAD;
- nothing in any report is untested, unreached, not run or skipped;
- the UX gate passes;
- the tracker's criteria are met.

### Untested is not accepted

When any report — implementer, reviewer or verifier — says something could not be tested, reached or run:

1. Do not accept or merge the task.
2. Find the reason yourself: read the report and the code, the QA seed's scenarios
   (`cargo run -p orchester-app --example seed -- --list-scenarios`), `examples/qa_app`, the fake agents, the
   stand-in server; reproduce it when cheap; or send a `research` child.
3. Act on what you find:
   - **The child did not try properly** (wrong command, gave up early): a new child on the same task with the exact
     way to test it.
   - **Something is missing** — a seed scenario, a QA app state, a fake agent's behaviour, a stand-in route, a test
     helper, a design frame, a tool: create a task for it in the module (`create_task`; title `QA: …`, `Design: …`
     or `Test support: …`; the brief says what is missing and which task waits), set the waiting task's
     `blocked_reason: "waits for ORC-<m>: <what>"`, drive ORC-<m> through its whole cycle to merge, then resume the
     waiting task: a new verifier or implementer tests what was missing. Log both ends.
   - **Truly impossible on this machine now** (macOS-only behaviour, a person-only step while the person is away, a
     paid external service): the task stays open with `blocked_reason: "needs the person: …"`, and `decisions.md`
     says what exactly to run and why you could not. It is never marked done.

### The UX gate

Send work back — or the design to a design agent — when any of these is true:

- the screen does not tell the person which state they are in, what happened, or what to do next;
- more than one primary action, or an action whose words do not say what it does;
- jargon, internal ids, raw error codes or developer words in the UI;
- a dead end (no way forward or back), a silent no-op, a control that looks active and does nothing;
- a destructive or irreversible action without confirmation or undo; work lost on navigation;
- layout that jumps or flickers on load, on hover or on focus — a control that appears on hover takes space reserved
  for it, it never pushes text aside, even when the design draws the shift; lost focus, a scroll reset, content hidden
  without a sign;
- the same thing done differently than elsewhere in the app (tabs, menus, dialogs, chips, empty states);
- an error with no way out; loading without bones; an empty state that does not explain itself;
- behaviour that differs from what its label promises.

A decision is never accepted only because the brief or the design says so. When the design itself carries the
problem: with the person present, `AskUserQuestion` (two options, the clearer one first with "(Recommended)", each
saying what it costs); with the person away, choose the clearer behaviour, have a design agent draw it, build that,
and record it in `decisions.md` with both the drawn and the built screenshots.

## Working without the person

Do not stop to wait. Decide what would block by these rules, in order: what the design shows; what the app already
does for the same thing; what surprises the person least; the smaller and more easily reverted change. Log it, record
it, continue with everything else.

What waits for the person (do the rest meanwhile, and list it under *Needs the person*): merging or pushing to
master; releases and versions; committing the `.pen`; deleting tasks; changes in the platform repository or to the
server's contract pin; anything outward-facing or irreversible; steps a QA copy refuses as person-only (unless the
AGENTS.md exception for a QA copy you started yourself applies).

`decisions.md`, one file per module, newest first under each heading:

```markdown
# <Module> — decisions and deviations for the person

## Needs the person
- [ ] **N-003 · ORC-866 · 2026-11-10** — <what you need from the person, in one line>
  - Why I could not: …
  - What to run or decide: …

## Decisions and deviations taken
- [ ] **D-007 · ORC-864 · 2026-11-10** — <what was decided, in one line>
  - Design: … (frame `XXXXX`). Built: …
  - Why: …
  - Alternatives: …
  - To revert: <commit or files>
  - ![design](shots/orc-864-XXXXX-design.png) ![app](shots/orc-864-XXXXX-app.png)
```

Every deviation from the design, every UX choice you made in its place, every brief you corrected in substance, and
every tracker refusal gets an entry. The person ticks an entry to accept it; a comment overrules it and becomes a
task. At the end of each session tell the person how many entries are open, with the file's path.

## Children that fail

- **No result, crashed, out of context or out of usage**: look at its slot (`git status`, `git log`), and stop any
  QA copy or cargo it left running there, by pid. Commits
  stay. An uncommitted diff that builds and is in scope is committed by the next child; otherwise save it as a patch.
  The next child gets the same brief plus "done so far: …".
- **Hung**: one `task_status`; then `task_cancel` and replace.
- **Broke a rule** (pushed, touched master, the main checkout or the installed app, killed processes it did not
  start, ran unfiltered tests): cancel it. Undo only your own branch's damage, with new commits; never destroy anyone
  else's work. Record anything beyond the module under *Needs the person*.
- **Implementer and reviewer disagree**: a fresh reviewer gets the finding, the implementer's answer and the code
  (not your view) and decides.
- **Fix rounds**: at most two per task. Then, with the person present, ask; away, park the task (`blocked_reason`),
  record it, move on.
- **Too large**: merge finished steps only if each passes the gate on its own and leaves nothing half-built visible
  to the person; the rest becomes a new task in the module.

## Merging

In `.tmp/worktrees/claude-module-<module>`:

1. `git fetch`; `git merge --ff-only origin/module/<module>`. Not a fast-forward (pushed from elsewhere): merge it,
   run the check. Never force-push.
2. The task branch must contain the module HEAD. If not, a child merges the module into the task branch and re-runs
   the tests: code conflicts are product code and go to a child. You resolve only conflicts in docs.
3. `git merge --no-ff claude/orc-<n>-<name> -m "Merge ORC-<n>: <what>" -m "Co-Authored-By: Claude Opus 5.5
   <noreply@anthropic.com>"` (the attribution line of the current session).
4. Run the affected tests and the check. Red after the merge though both sides were green: `git revert -m 1` the
   merge, and a fix task.
5. Known failures on master are in `tools/check-tests-known-failures.txt`. A failure that predates the task but is
   not listed there goes into `STATE.md` with the commit where it appeared. Anything else is new.
6. Push the module branch.
7. Tracker status, `decisions.md`, `pipeline.md`, `STATE.md`.
8. Clean up the task's workspaces; its slot goes to the next task.
9. Start what the merge unblocked; a task that waited on this one (see [Untested is not accepted](#untested-is-not-accepted)) resumes now.

**Master**, only when the person says: in a detached temporary worktree at `origin/master`, merge the module, show
the diff against the module and check it has no `version` change in `crates/desktop-app/Cargo.toml` the person did
not ask for, push `HEAD:master`, remove the temporary worktree. Then `git merge --ff-only origin/master` in the main
checkout; if the person's local edits refuse it, leave it and tell them — never stash, reset or overwrite their
edits.

## Design (Pencil)

- The URI is always the main checkout's `crates/desktop-app/design/orchester.pen` as `file:///…`. The first
  `execute` of every design or verification child reads the board by id and stops if it is missing: Pencil answers
  silently from the active tab for a path it has not open, and board names repeat.
- One design child on a document at a time; you do not edit while one works. **Check first whether the person is
  working in Pencil** (`get_app_state`: the boards in view); while they are, ask before a design child edits the live
  document.
- **Export the module's frames once**, at its start, into `shots/design/<frame id>.png` with each frame's sizes (a
  condensed `Get` of the frame to the depth a verifier needs, as JSON beside the PNG); again for a frame that changes.
  A design verifier compares with the live document when Pencil answers and with these exports when it does not, and
  says which it used. Children often find the Pencil MCP not connected (`transport not connected to app`) while yours
  is.
- `save` only when the person allowed it for this session. Otherwise the design child exports a PNG of every frame
  it changes into `shots/` and reports node ids; after a crash, check which ids survived and redraw from the exports.
- Never revert the person's unsaved or uncommitted edits in the document; never commit the `.pen`.

## Platform and contract

A task that needs `orchester-api` (`docs/cloud-api.md`) changes it here first, in the module, and builds the desktop
side against the stand-in. Moving the pin in `ivanzaida/orchester-platform` and server work wait for the person.

## macOS

A diff touching `cfg(target_os = "macos")` code or macOS scripts: the child names that code; the macOS check is
listed under *Needs the person* (CI's macOS job runs by hand), never claimed as run.

## Resuming

Update `STATE.md` after every spawn, report and merge, not only before a switch:

- running children (never more than five): `taskId`, `link`, task, role, round, slot, branch;
- the slots: which task or batch holds each, which are free, which are warming (the background task's id);
- branch heads (module, task branches); the waves left; blocked tasks and what they wait for;
- *Needs the person*; known pre-existing failures; the person's rules for this module;
- the tracker writes still pending.

After a compaction, a new session or an account switch: read `STATE.md` and the tail of `pipeline.md`, then
`git worktree list`, the tracker (`get_module`) and `task_status` of the children listed as running; reconcile, log
what changed, and continue.

## Ending a module

1. Every task is done, or parked under *Needs the person*.
2. Merge `origin/master` into the module and run the check.
3. All slots handed back clean and detached. They stay warm for the next module, and are removed with
   `git worktree remove` only when the person says so or disk runs short. `STATE.md` is final.
4. Tell the person: what merged (SHAs), what is open in `decisions.md`, what waits for them (master, the `.pen`,
   macOS, platform).

## Reporting to the person

Short updates in the person's language: what started (each child's `link`), what finished with its verdict, what
merged (SHA), new entries in `decisions.md`, what waits for them. Never claim a test you did not see run; say what
was not run.
