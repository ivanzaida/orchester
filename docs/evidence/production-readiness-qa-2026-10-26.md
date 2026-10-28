# Production readiness QA — 2026-10-26

Exploratory QA of `crates/desktop-app` (master `056471073`) on isolated QA copies (`tools/qa-app.ps1`), driven
only through the copies' embedded MCP server. Three areas: first run and setup, the task → run → merge lifecycle
with the fake Codex, and data-heavy screens with error states. About 40 scenario launches. No panic or error was
logged by any copy (`app.err.log` empty everywhere). Screenshots (git-ignored): `.tmp/qa/shots/{setup,runs,data}/`.

Severity: **major** = a person gets stuck, is misled, or loses work; **minor** = wrong or confusing but has a way
around; **polish** = wording and visual details.

## Major

### Recovery and failure states
- **R1 · Failed only at *Create pull request* → reviewed work cannot be delivered.** `interrupted -FakeAgents`,
  KON-21 *Continue run*: steps 01–05 complete, 06 fails (`gh pr create`: no git remotes). Offers are only
  *Return to 03 Implement*, *Run again*, *Remove worktree*; *Local integration* says "available after the run
  succeeds". No retry of the PR step, no local merge. Summary says only "Its command failed".
  (`runs/int2-kon21-continued.png`)
- **R2 · Configuration errors are shown as agent failures, and burn retries.**
  - `interrupted` KON-12 *Continue run* with the profile's connection gone: "Implement broke again after its
    restart", automatic restart spent (`CouldNotStart` twice), *Allow 1 retry & continue* can only fail again.
    The cause ("connection no longer exists") is only in Conversation, no link to Connections.
    (`runs/int-kon12-continue.png`)
  - `tasks` KON-8 *Start run*: "The runner could not do this; its log has the details"; the log has
    `Preflight{ConnectionMissing}` for every step. Button stays enabled, no link. (`data/task-kon8-startrun.png`)
- **R3 · Interrupted-runs banner is stale; *Continue all* reports false results.** After continuing KON-12 the
  banner still lists it and offers *Continue all 3*; status bar says 3, metric says 2. *Continue all* then says
  "2 of 3 continued" though both failed at once. (`runs/int-overview-after-one.png`, `runs/int-continue-all.png`)
- **R4 · Pause never completes when the agent reaches no checkpoint.** `live` KON-5 *Pause run*: *Pausing…* for
  over a minute, status *Running*, *Stop turn* gone, pause cannot be withdrawn; only *Cancel run* gets out.
  (`runs/kon5-pausing-40s.png`)
- **R5 · One unreadable record takes down unrelated screens.**
  - `tasks-unreadable`: one task with an unknown status → Tasks, Overview and New task all fail with "Tasks cannot
    be read"; the task is not named (not in the log either), *Try again* can never succeed, no Open log.
    (`data/tun-tasks.png`, `data/tun-overview.png`)
  - `agents-unreadable`: a corrupt agent profile is reported on Tasks and New task as "Tasks cannot be read";
    Agents offers no way to find or remove it. (`data/aun-tasks.png`, `data/aun-agents.png`)

### Status that contradicts itself
- **S1 · A failed connection check is announced as success.** Inspector: *Could not run / Test failed*; status
  bar: "✓ Checked Claude · Kontur". After *Test connection* and *Refresh models*, `connections` and
  `api-providers`. (`setup/conn-claude-refresh.png`, `setup/conn-codex-test.png`)
- **S2 · "Running now" counts runs that are not running.** `interrupted`/`tasks`: "Running now 3 · 3 runs
  interrupted" with nothing working (still after *Don't continue*); `live`: runs waiting for approval/answer
  count as running. The close dialog lists them as "Running" and omits a stopped turn. `usage`: "Completed today
  0" next to "3 completed". (`runs/overview-all-running.png`, `data/tasks-overview.png`, `data/use-overview.png`)
- **S3 · Interrupted runs without a runner look live.** `task-states -NoRunner` KON-11: notice says "Restart the
  runner", the card says "the run continues once one is", the sidebar shows a ticking timer; active agent time
  1008 min against a 90 min limit, "Running with · Builder". Same on KON-3, KON-7, `memory`.
  (`data/states-kon11.png`, `data/states-kon3.png`)
- **S4 · After *Stop turn* the composer is addressed to the Orchestrator** while the transcript says Builder waits
  for your message; the recipient chip opens nothing. (`runs/kon5-stop-turn.png`, `runs/kon5-recipient-menu.png`)
- **S5 · *Hold new starts* refuses *Run*, and *Release* does not start the task.** Message says "Release them in
  Tasks to start it"; after Release the task stays Draft. Design says first runs wait while held.
  (`runs/empty-kon2-held.png`)

### Small window and keyboard
- **L1 · At 960×640 logical, main pages lose content.** All projects is wider than the window, centred, title
  reads "ects", *Add project* off-screen; New task's form is a fixed 1080 px and clipped both sides, *Create &
  run* cut; Tasks drops the title column (rows indistinguishable); Overview rows lose title and key, tabs "Que…";
  sidebar *Skills* cut in half. No horizontal scroll. At 700×500 the sidebar takes half the window and words
  break mid-word; there is no minimum window size. (`setup/small-allprojects.png`, `setup/small-newtask.png`,
  `data/small-tasks.png`, `data/small-overview.png`, `setup/tiny-overview.png`)
- **L2 · Tab order on New task skips almost every field.** From Title, Tab cycles Title → *Add criterion* →
  *Don't merge automatically* → title bar + → back. Description, source, pipeline, limits, Create are never
  reached; the focused switch is hidden under the sticky footer with no focus ring.
  (`setup/newtask-tab-focus-merge.png`)

## Minor

### Setup and connections
- *Refresh models* on failure wipes known models and version ("Unknown"). (`setup/conn-claude-refresh.png`)
- "did not answer within 30 seconds" for a failure after ~2 s; the same result is shown twice.
- Profiles on a broken connection show no warning: Agents says "0 need a connection"; Overview *Ready to run* names
  the unused Claude connection. (`setup/conn-agents-after-fail.png`)
- No way to pick a CLI not on `PATH` (*Choose file…* unavailable) — a blocker for macOS Finder launches.
  (`setup/conn-add-provider.png`)
- Duplicate project name/key accepted (two "Kontur · KON" tabs), and renaming is "not available yet".
  (`setup/all-projects-two-kontur.png`)
- A missing folder is reported as "not the root of a git repository" (seen through `add_project`).
- Forward-slash and backslash paths mixed on Windows. (`setup/all-projects-two-kontur.png`)
- Blank connection name → "Enter a project name.". (`setup/conn-add-runs-provider.png`)
- New agent: "Unsaved changes" before any input; model defaults to "Other model id…" though models are known;
  errors drawn left of the column, the name field not marked. (`setup/api-new-agent-empty-save.png`)
- Settings shows "All changes saved" while the warning threshold is invalid.
  (`setup/settings-threshold-invalid-saved.png`)
- New task with a 500-char title: field says "at most 200 characters", footer says "add a task title".
  (`setup/newtask-long-title.png`)
- Locked controls without a reason: *Add GitHub connection*, Project settings › Memory, Settings › Devices; locked
  General fields look editable and show a literal "none". (`setup/project-settings-light.png`)
- "Copy a pipeline" is offered with nothing to copy and opens an empty sliver. (`setup/fresh-pipeline-copy-open.png`)
- Overview *Ready to run* rows (Agents / Pipeline / Connection missing) are plain text, no links.

### Runs and tasks
- One state, four labels for KON-1: "Ready for review" / "Completed" / "Ready to merge · Deliver" (no Deliver
  step) / "Run #1 completed". Canceled KON-7 shows "Ready to queue". (`runs/tasks-list.png`, `data/tasks-all.png`)
- *Ask agent* locked as "this app doesn't have the composer yet" although the composer exists and works.
  (`runs/kon1-summary.png`, `data/dec-kon1.png`)
- Paused notice says "Resume run continues Research" while Research is complete (Plan resumes).
  (`runs/life-kon2-paused.png`)
- *Cancel run*: one click, no confirmation, beside *Pause run*. (`runs/kon5-canceled.png`)
- Run #7's conversation shows run #5's message below its own entries, no run boundary. (`runs/kon5-stop-turn.png`)
- Run numbers vs branch names disagree (run #5 → `kon-5-run-1`). (`runs/kon5-run-again.png`)
- Durations include idle/interrupted time ("11 min" for ~1 s of work, "478 min"); a follow-up is "attempt 2".
- Held task KON-17: *Start run* enabled, then an error that does not name the blocking task.
- Token limit KON-5 "Used 500k of 500k" but Implement shows 0 tokens; stopped steps show 0. (`data/states-kon5.png`)
- Plan limit KON-19: no Continue, no reset time, no account switch. Daily budget KON-6: no link to raise it.
- Unreadable run KON-17: "The log has the details" but no Open log.
- Changes tab badge 6 vs header "4 files"; PR #137 header "14 files +412/−198" vs list of 2 files.
  (`data/changes-kon12.png`, `data/pr-137.png`)
- Blocked delivery *Retry* gives no feedback. (`data/delb-retry.png`)
- Merge from the merge queue refused with "Press Merge into the base branch yourself" while the button reads
  "Merge into main" (may be the automation gate; wording still mismatches). (`data/del-merged.png`)
- Question card: picking an option or typing an answer did not register, *Send answer* stayed disabled, no notice
  or log line (possibly the person gate — unconfirmed).

### Status bar
- Failure notices pile up and stay across all screens for 10+ minutes; dismissing one reveals an older one; *Open*
  does nothing; with several notices the bar truncates and other items collapse to icons.
  (`setup/settings-1.png`, `runs/kon5-sent-20s.png`)

## Polish
- Raw error on the approval card: `exec_command failed: CreateProcess { message: "Rejected(\"rejected by user\")" }`.
  (`runs/kon2-conv.png`)
- Internal names shown: `start_qa_app` in Project settings, *QA apps at once*; run budget max
  "9,223,372,036,854,775,807". (`setup/newtask-budget-max.png`)
- Text cut without ellipsis: worktree path, locked pipeline field, Agents version column, post-merge-conflict
  status. Plurals: "1 tasks", "1 allowed commands", "worktrees" for one.
- Add project: steps 2–3 always "Not available in this version yet"; one page named three ways.
- "Create multiple" shown disabled as "planned".
- Settings opens on Automation, not General; scroll bar over controls at 960 wide.
- Overview *Review*/*Answer* use a chevron-down but navigate away; unlabeled priority glyph on Overview rows.
- *Create pull request* disabled without a reason; "Write a message to send it." tooltip lingers; task row ⋯ menu
  has no *Run*; MCP server form keeps Save enabled while saying "Save unavailable".
- Sign-in gate sentence about *Create an account* / *Forgot password?* reads awkwardly.
- Starter pipeline "2 approval gates" with one gate icon; bare "Remove" under a missing source.

## What held up
- No duplicates from double-clicking *Run*, *Create & run*, *Start run #N*, *Merge into main*.
- Composer and New task text survive navigation; merges land on `main` and remove worktree and branch.
- Long transcripts (1,500 entries, 300-turn chats) open in ~0.5 s; a 6,248-line diff scrolls at ≤22 ms frames.
- Changes "too large" and "worktree removed" states, the unreadable-database error, the first-launch recovery
  banner and the close dialog's *Wait for agents to save* are clear and work.

## Not covered
- Every person-only decision: approve/reject, answering a question, approving a plan, *Allow 1 retry*, and the run
  after them (the consent switch refuses automation by design). Needs a person to turn on *Let automation answer
  approvals* in a QA copy's window, or a manual pass.
- Signing in (input is refused while signed out), sign out/lock, Accounts actions, Automation switches.
- The native folder picker (Add project, sources) — add-project validation was checked only via `add_project`.
- `pipelines` / `pipelines-empty` scenarios, bulk selection actions, OS toasts and badge, *Open runner log* /
  *Open file* (open external programs), light theme beyond Settings, Tasks and Overview.
- macOS.
