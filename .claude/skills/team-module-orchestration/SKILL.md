---
name: team-module-orchestration
description: How Claude orchestrates a module's tasks outside Orchester's pipelines — spawning Sonnet implementers, independent reviewers and design verifiers in their own worktrees, merging into the module branch, and keeping the log. Use when the person says to continue a module "outside Orchester", to start, review or merge module tasks, or when resuming orchestration on another account.
---

# Orchestrating a module outside Orchester

You (Opus, the main thread) orchestrate. You write no product code. You hand out tasks, make routine decisions,
merge, and keep the log. Workers are child agents.

## Sources of truth

- Task briefs: the Orchester tracker (`get_task` over the app's MCP). If the app is not running, read them read-only
  from the DB copy: `.tmp/orchestrator/dbcopy/orchester.db`, table `tasks` (`number`, `title`, `brief`), with
  Python run with `-I` (`python3` on macOS, `python` on Windows) and `?mode=ro`. Never open the live `.orchester/orchester.db` for writing.
- Real task state: `docs/design/team-configuration-handover.md` on the module branch. Statuses on the team server
  cannot be changed over MCP (`decision_required`), so the tracker shows Backlog.
- Model and decisions: `docs/design/team-configuration.md`.
- Decision log: `.tmp/orchestrator/pipeline.md` (append one line per event or decision, and the person's rulings
  under a `## Person (date): …` heading).
- Resume snapshot: `.tmp/orchestrator/STATE-<date>.md` (running, branch heads, left, waiting for the person, rules).
  Update it before a session or account switch.

## Spawning workers

- Use `delegate_task` with
  `target: { providerInstanceId: "<the Claude provider instance of the current account>", model: "claude-sonnet-5-5" }`
  and a `role` (`implementation`, `review`, `design`). Take the instance id from the tool's own list for the account
  the person currently runs on; ask if unsure. Never copy an id from an old log or snapshot.
- A child sees none of this thread: the brief is all it knows. End the turn after spawning. Completion wakes you; do
  not poll. Use `task_status` only for a long-silent child.
- To correct a running child (a new design node, a path fix), use `t3_thread_send` with `mode: "steer"`.
- Every review or re-check is a NEW delegate, never a message to an old one.
- Load: at most ~3 children that build Rust at once (a desktop machine cannot take more). Design and read-only
  children don't count. Check the machine's load before adding builders (`uptime` on macOS; on Windows
  `Get-CimInstance Win32_Processor | Select LoadPercentage`) and its free disk: each worktree's `target/` takes
  20–40 GB.

## One task's cycle

1. **Worktree.** You create it yourself, per AGENTS.md:
   `git worktree add .tmp/worktrees/claude-orc-<n>-<name> -b claude/orc-<n>-<name> module/team-configuration`
   Dependent tasks that would otherwise conflict can share one worktree and agent (e.g. ORC-844+845).
2. **Implement.** A fresh Sonnet gets a brief containing:
   - the tracker's brief verbatim;
   - your and the person's decisions;
   - what is merged that it builds on;
   - which parallel branches touch nearby code ("keep X, don't move Y");
   - the rules:
     - work only in its worktree;
     - commit with conventional messages ending in
       `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`;
     - never push;
     - never touch master or the main checkout;
     - read and follow `AGENTS.md` and the skills `rust-code-quality`, `lurq-ui` (with references), `orchester-ui`;
     - `CARGO_BUILD_JOBS=4`;
     - every `cargo test` with `-p <crate> --test <binary> <filter>`;
     - the check: `bash tools/check.sh` on macOS, `powershell -File tools/check.ps1` on Windows (list the known
       pre-existing failures so it names only new ones);
     - its own screenshots only from a QA copy (`tools/qa-app.ps1`, run with `pwsh` on macOS; data under
       `.tmp/qa/orc-<n>/`), never the installed app or its data;
     - stop its own processes by pid;
     - no design verification of its own work.

   Its report must give: SHAs, what was built per frame or step, how to open each state in a QA copy, tests run with
   results, the check result, and what is not done or deviates, plainly. If the task is too large, it finishes the
   steps in order, keeps each commit green, and says where it stopped.
3. **Code review.** A fresh Sonnet that did not implement. It gets the brief and the range
   (`git log/diff module/team-configuration...HEAD` in that worktree), never your conclusion. It edits nothing,
   re-runs the tests the claims rely on and the check, and returns a verdict PASS or FAIL with findings ranked
   blocking, major or minor, each with file:line and a concrete failure scenario.
4. **Design verification** (any user-visible change). A separate fresh agent following
   `.claude/skills/design-verification/SKILL.md`: a Pencil screenshot of the node and an app screenshot of the same
   state at the same size, compared region by region. Done means no blocking or major mismatch.
5. **Fix rounds.** Blocking or major findings send a new implementer to the same worktree with the findings. At most
   2 rounds, then ask the person. You may merge a small, targeted, test-proven fix without another review; log that
   you decided so.
6. **Merge.** In `.tmp/worktrees/claude-module-team-config` (branch `module/team-configuration`):
   1. `git fetch && git merge --ff-only origin/module/team-configuration`;
   2. check that the task branch contains the module HEAD, or merge the module into it first and re-test;
   3. `git merge --no-ff -m "Merge ORC-<n>: <what>\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"`;
   4. run the affected tests and the check;
   5. push the module branch;
   6. update the handover file;
   7. `git worktree remove` the task's worktree.
7. **Not-done items.** Anything a report says was not done (design missing, verification not run, a locale, a state
   not reachable) becomes a new task in the module, driven to the end. When it is merged, tell the run that waited.
8. **Unblocked tasks.** After a merge, start the tasks it unblocked.

## Design (Pencil)

- Document: `crates/desktop-app/design/orchester.pen` in the MAIN checkout, layer `acFkz` "Agents · v2" (sections A, M,
  C, R, L; notes).
- Always pass `filePath`/`uri` as the absolute path of that file in the main checkout of THIS machine, as a
  `file://` URI (spaces as `%20`). Find the main checkout with `git worktree list` (its first line) — never use a
  worktree's copy. Pencil may have several copies of the document open (other clones on the same machine) with the
  same top-level frames; a path it does not match answers silently from another copy. So in the first `execute`,
  check that the layer `acFkz` exists, not just the frame names, and stop if it does not. Put the resolved URI and
  this check in every brief that touches the design.
- A missing design gets its own design agent. It changes the `.pen` only through Pencil, matches the existing frames'
  components and variables, adds a note node with the rule it draws, exports screenshots to
  `.tmp/orc-<n>/design/`, and reports node ids.
- You save with `pencil save` when the person asks for a commit, then check that the new node ids are in the file.
  Commit the `.pen` only when the person says so.

## Decisions

- Decide routine continuation yourself and don't block: round size, merge order, whether a small fix needs
  re-review, a follow-up task.
- When an agent finds a gap in the product model (a rule the design and the code disagree on), ask the person with
  `AskUserQuestion`: 2 options, your recommendation first with "(Recommended)", each saying what it costs. Then pass
  the answer into the next brief as "Decisions by the person (date) — follow them", and start a design agent if the
  design must change.
- Log every decision in `pipeline.md` and tell the person in a line.

## Branches and push

- Work lands in `module/team-configuration` in both repositories: desktop, and platform `orchester-platform` (its own
  worktrees there).
- You may push the module branches, e.g. when the platform needs the contract (pin move).
- Push or merge into `master` only when the person tells you to. Do it in a detached temp worktree at `origin/master`:
  merge, show the diff against the module, push `HEAD:master`, fast-forward the main checkout's master (its local
  edits kept), remove the temp worktree.
- End of module:
  1. merge `master` INTO the module and run the check;
  2. delete the handover file;
  3. tell the person.

## Reporting to the person

- Short updates in the person's language: what started (with each child's `link`), what finished, the verdict, what
  merged (SHA), what waits for them (decisions, a `.pen` to commit, deploys).
- Never claim tests you didn't see run; say what was not run.
