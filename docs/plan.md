# Work plan

The backlog of Orchester work that is known but not started, written so an agent (Codex or Claude, inside or outside
Orchester) can pick up one task without extra context. It lists the goal of developing Orchester inside Orchester first,
then the backlog by area.

## How to take a task

- Read `AGENTS.md` and follow it. It links the skills to read before Rust code (`rust-code-quality`) and before any
  visual change (`lurq-ui`, `orchester-ui`).
- Take one task. Its **Done when** list is the acceptance criteria; do not widen the scope. Report unrelated findings
  instead of fixing them.
- **Non-UI** tasks need `powershell -File tools/check.ps1` and the named filtered tests.
- **UI** tasks also need an independent design verification (`AGENTS.md` rule 15): a second agent that did not write
  the change compares Pencil and app screenshots. Inside an Orchester run that means a separate *Verify* step with its
  own agent profile, exact-PID app capture and host-side focus check. D4 proved that verification path for an
  already-authored UI candidate; authoring a UI change inside Orchester has not yet been demonstrated.
- When a task is done, move it to *Done* at the end of this file in the same change, with the commit or branch.
- Tasks marked **needs decision** wait for the user; do not start them.

## Work in progress · 2026-09-29

| Task | Branch | Verified progress and next gate |
| --- | --- | --- |
| D5 | `codex/d5-live-repro` | Independent Windows tests, code review, and four exact-PID app runs cover cannot-measure and low-space errors, truthful queued state, same-task retry, and inert queued pipeline. Pencil now has matching error/retry frames. Same-state comparison and full runner macOS check remain; candidate unmerged. |

## D · Developing Orchester inside Orchester

What is already in place: runs in their own worktrees, pipelines with agent and command steps, limits, questions and
the task conversation, Ready for review, *Merge into main* into the user's checkout, worktree cleanup, project
Environment variables, project MCP servers, tabs, and the installed stable build (`docs/stable-build.md`).

### D1 · Set up the Orchester project in the stable build (user, manual)

1. In the stable build, add `H:/projects/orchester` as a project. First make `.orchester/` ignored by Git for this
   checkout (for example in its local `.git/info/exclude`); the runner refuses to create a run worktree otherwise.
2. Project instruction: *Read AGENTS.md at the repository root and follow it. Before writing Rust, read
   `.claude/skills/rust-code-quality/SKILL.md`. Take tasks from `docs/plan.md`.* Claude Code is started with
   `--setting-sources ""`, so it does not load `CLAUDE.md` on its own; Codex reads `AGENTS.md` itself.
3. Agent profiles: **Implement** (Codex, workspace write, network per the capability table) and **Review** (a different
   profile, read-only), so the reviewer is never the author.
4. Pipeline *Orchester change*:
   Implement → command `powershell -File tools/check.ps1` → command with the task's filtered `cargo test` lines →
   Review → Ready for review.
   Set the Implement **step** to *Can write* as well as the profile: a new agent step starts *Read-only* and can narrow a
   write profile. Keep Review read-only.
5. Environment: `CARGO_TARGET_DIR=<repo>/.orchester/target`; then, in Agents › Permissions of each profile that
   builds, add it to *Extra writable folders* (the tab offers it).
   Without it every run builds its own `target/` in its worktree (several GB, several minutes the first time).
6. Limits: start at about 3M tokens and 45 minutes of active agent time per run.
7. *Merge into main* merges into the user's checkout, which must be on `master` with no uncommitted changes. Do not
   merge from Orchester while another agent is merging branches into `master` by hand.

### D5 · Check free disk space outside the agent CLI

In the first D3 run, Codex 0.139.0 with `-a never` declined `Get-PSDrive`, `fsutil volume diskfree H:` and `Get-Volume H`
in a read-only step. A task that required that check stopped before cargo. Add an Orchester host-side free-space
preflight for builds using a shared target, with a clear low-space result on Windows and macOS, so the agent does not
depend on shell commands that its sandbox may decline. Evidence: `docs/evidence/d3-first-real-run-2026-09-29.md`.

The implementation on `codex/d5-free-space` checks an absolute project `CARGO_TARGET_DIR` on the host before creating
a run or worktree. It returns typed, localized errors when the target's volume cannot be measured or has less than
10 GiB free for a cold target or 1 GiB for a populated one. Relative targets are skipped. A low-space target also
blocks a documentation-only run because the runner cannot know whether an arbitrary step will build.

**Done when** the filtered boundary and error-path tests, `tools/check.ps1`, independent review, and Windows host
measurement pass; the runner macOS target passes cross-check or a Mac smoke test; and a separate agent compares the
new visible error states with exact Pencil frames and app screenshots of the same size and state.

**Status 2026-09-29: In progress.** A current-master candidate at `codex/d5-live-repro` passed filtered Windows
tests, independent code review, `check.ps1`, and four independent exact-PID app runs. A regular file beneath the
absolute Cargo target reproduced the cannot-measure error before run/worktree creation. The first run exposed a
misleading *Not saved yet* state after the task had been queued. The candidate now shows the queued state truthfully,
freezes its fields and pipeline, and retries the same task/run key; the second run verified one successful run after
the target was corrected, and the third verified the inert pipeline and visible error. A fourth run used an empty,
absolute target on H: with 2.643 GiB free, below the cold-target 10 GiB threshold. It showed a readable low-space
error, left exactly one new task queued, and created no run or worktree. Ambiguous runner replies now remain
explicitly unconfirmed. Evidence: `docs/evidence/d5-live-reproduction-2026-09-29.md`.

The candidate stays unmerged: the new Pencil low-space and cannot-measure frames need a same-state app comparison.
The macOS `fs4` dependency check passed, but a full `orchester-runner` cross-check needs macOS SDK headers or a Mac.
The proposed design path is in `docs/evidence/d5-design-path-2026-09-29.md`; earlier checks are in
`docs/evidence/d5-status-2026-09-29.md`.

Pencil (drawn 2026-09-29): `WOdDe` (02.09C, cannot measure), `WmW1G` (02.09D, low space), `l801hD` (02.09E, retrying the
same run). They include D4's New Task changes (View pipeline, the approval card, the instructions footer), which the D5
branch does not have yet; compare the header, description, pipeline box and the actions with their error.

## T · Tests and runner

### T4 · A listing survives a row whose id cannot be parsed (non-UI)

A run row whose id or task id cannot be parsed still fails the whole listing
(`crates/store/src/project_db/runs/load.rs`), because it cannot say which run it is. Decide with the store owner
whether such a row is reported as a project-level "unreadable rows" error that still lists the others. **needs
decision**.

## M · MCP servers

### M1 · Confirm MCP behaviour in a real run (non-UI)

Confirm with model turns whether Codex under `-a never` lets the agent call an MCP tool, whether
`shell_environment_policy.exclude` keeps secret variables out of the agent's own commands, how Codex reports a server
that failed to start in a signed-in run, and whether Claude's `mcp__<name>` allow rule is enough. **Done when** each is
confirmed or fixed, with the observed CLI output saved as a fixture and parser tests updated
(`crates/core/src/connection/mcp/`).

### M6 · Environment names in secret references (needs decision)

Environments display in lower case (`@staging`, *(staging)*) where the design writes *Staging*, and typing `@Staging`
is refused (`Environment::as_str` in core). Decide whether names are case-insensitive with a display form; ties in with
the Secrets decision under P.

### M4 · Not built yet (needs decision)

Tool discovery and OAuth sign-in for MCP servers, narrowing servers per pipeline step, changing a running run's servers,
and MCP tools in the Tools library.

## U · Screens

### U10 · Pull requests screen jumps while it loads (UI)

Opening the Pull requests tab shifts its content two or three times
(`crates/desktop/src/screens/project/pull_requests/mod.rs`):

1. The stored inbox is read from the project database asynchronously (`actions::load` → `ctx.future`,
   `actions.rs`). Until it arrives, `inbox` is `None`: the body is an empty column and the Open tab has no count.
2. When the stored inbox arrives, the Open tab gains its count (`synced.pull_requests.len()`), so it widens and pushes
   the other tabs right, and the list appears below. For an unlinked repository the whole tab row disappears
   (`tabs` is skipped for `Inbox::Unlinked`), which moves the body up.
3. `create` also starts a GitHub sync through `gh` at once. While it runs the topbar sync status text changes width,
   and afterwards the list and count may change again.

**Done when** the screen keeps a stable layout from the first frame: the heading, tab row and body keep their place
until the stored inbox is known (no empty-body frame followed by a list; no tab row that appears and then vanishes),
the Open tab reserves room for its count so a count arriving does not move the other tabs, and the topbar sync status
does not shift the Refresh button when its text changes. A sync that changes the list may still add or remove rows.
Tests: a headless `form_keyboard` test that records the tab and body bounds before and after the stored inbox and a
sync arrive (fake `gh`), plus the existing `pull_request` tests. UI change: independent design verification against the
Pull requests frames (`DF8II` tab row, 08.D `dmib5`) is required.

## L · After lurq 0.30.0

The tab and text-input fixes shipped in lurq 0.30.0, the outside-press fix for window chrome in 0.30.1, the
select-menu width option in 0.30.2, and the verifier window focus option in 0.30.3. This workspace uses 0.30.3.

## P · Product decisions waiting for the user

- **Secrets:** fixed Development / Staging / Production environments; archive a secret instead of deleting it.
- **Permissions:** on the agent profile page the 22px reason indent lines up with neither the radio nor the label.
- **All projects:** the RECENT list can show a project whose data cannot be read.
- **Pipeline history:** show who published a version (the store records no author yet).
- **App log:** an *Open app log* action (no design yet).
- **Import:** where a copy's provenance is shown (recorded, not shown).
- **Tabs:** reopening a closed tab (not designed).
- **Project bar Search and notifications:** Pencil `CsB3h` shows Search and a bell, while the app omits them.
  Define working behavior and data sources before adding controls; do not add inert placeholders.
  Evidence: `.tmp/verify/u9/report.md`.

## Later

- **macOS:** build, install and smoke-test on a Mac (`docs/stable-build.md`, macOS section). All macOS code is only
  cross-compiled so far.
- **Project settings:** disk use of run worktrees.

## Done

ORC-16 · Run again · branch `claude/orc16` (a failed or cancelled task starts its next run from the first step in a
new worktree and branch, with the latest published pipeline version and the task's limits; the rule is core's
`run_again`, the runner's `RunAgain` command is idempotent by key and can first remove the run before's worktrees
with the *Remove worktree* rules; the task page offers *Run again* beside *Remove worktree*, the confirmation
`R5y5x`, and while blocked the action stays unavailable with the reason under the notice. Core `run`, store `runs`,
all runner binaries, `form_keyboard ::`, `tasks`, `i18n`, `theme` and `tools/check.ps1` passed. An independent
exact-PID comparison with `zM693`, `M1upHe`, `R5y5x` and `iE8eQ` reached every state and found no blocking or major
mismatch; the failed/cancelled Summary body predates ORC-16 and still differs from `zM693`).

U6 · Verify conversation states and direct recipient timing · branch `codex/u6-final-port` (an independent agent
compared exact-PID Interrupted `sYuiX`, Not started `Ok14G`, real Live recipient menu `cj2a2`, and Paused/Stopped
component captures at matching sizes; none had blocking or major visual mismatches. A later-agent message initially
promised delivery before Builder's next action, which the runner cannot guarantee. The corrected Paused and real
Live Builder→Reviewer states now say it waits for Reviewer's next turn; independent recheck passed conversation
8/8, i18n 5/5 and `tools/check.ps1`, with no own app focus in 1,343 host samples. Pencil has no separate saved
later-recipient message frame, so its exact copy lacks a literal same-state design comparison; the verified line
fits the saved panel geometry. Evidence: `docs/evidence/u6-conversation-states-2026-09-29.md`.)
M1 · Confirm MCP behaviour in signed-in CLI turns · branch `codex/m1-claude-allow-rule` (Codex 0.139 under `-a never`
called a probe MCP tool, kept a test variable out of agent shell commands, and reported an unnamed startup warning
for a broken server while the turn completed. Two repeat unannotated calls succeeded after one unexplained
cancellation. After sign-in, Claude Code 2.1.89 called `mcp__m1probe__ping` with `--allowedTools mcp__m1probe` and
no other tools; the turn exited 0 with the inert result and zero permission denials. Observed CLI fixtures and parser
tests were updated. The independent Claude fixture test passed 1/1 and `tools/check.ps1` reported OK. A full
Orchester pipeline run was not part of this probe. Evidence: `docs/evidence/mcp-real-run-2026-09-29.md` and
`.tmp/verify/m1-claude-allow-rule/report.md`).
U8 · Align agent profile header lifecycle actions with Pencil · branch `codex/u8-final-port` (Save draft appears only
for unsaved edits, while Publish follows the saved-draft state. An independent verifier matched all five saved
1440 × 960 header frames `dWnw7`, `t2wCI`, `Jw98x`, `WPmtS` and `T1rjv` to source-matched exact-PID app captures at
2160 × 1440 physical px, with no blocking or major header mismatch. Named agent form tests passed 4/4,
`tools/check.ps1` passed, and the host watcher recorded 931 samples without app focus. Fixture-specific profile
bodies differ from Pencil and were not claimed as full-screen parity. Evidence:
`docs/evidence/u8-header-states-2026-09-29.md` and `.tmp/verify/u8-final-port/report.md`).
U2 · Guard merge and worktree removal while integration runs · branch `codex/u2-merged-fix` (project/run-scoped busy
state keeps conflicting controls unavailable, the Merging button retains its width, and a merged run shows its
historical branch with a fully visible `· deleted` suffix. Independent real-Git integration tests passed 11/11;
the corrected candidate passed the focused real-Git test, i18n 5/5 and `tools/check.ps1`. Exact-PID app comparisons
to Pencil `jnagB`, `cE50X` and `bMJUA` found no blocking or major mismatch; a host watcher sampled the final app
381 times without it taking focus. Pencil has a button specimen but no full in-flight merging page. Evidence:
`docs/evidence/u2-merge-worktree-controls-2026-09-29.md`).
M5 · Show who reaches a remote MCP server and wrap its Authentication value · branch `codex/m5-final-port`
(remote inspector adds *Reached by*, aligns property rows to the top, and lets only the long Authentication row
grow. Independent same-state 2160 × 1440 Pencil `a4lm6D` / exact-PID app comparison found no blocking or major
mismatch; the value was fully visible on two lines and sections below aligned within 2–3 physical px. Named MCP,
typography and i18n tests and `tools/check.ps1` passed; the host no-focus watcher passed. Evidence:
`docs/evidence/m5-remote-inspector-2026-09-29.md`).
U3 · Fit the Claude mechanism line and enforcement panel to Pencil · branch `codex/u3-final-port`
(shorter English copy and two theme typography roles align the one-line mechanism, row spacing and wrapping in the
300-wide panel. Independent exact-PID same-state comparison to Pencil `CIeqc` found no blocking or major mismatch;
the full app panel is 300 × 492.7 logical px versus 300 × 495 in Pencil. The 2.3px residual and pre-existing heading
copy are minor. Independent theme/i18n tests and `tools/check.ps1` passed; a separate bounded host watcher confirmed
the own app stayed out of focus. Evidence: `docs/evidence/u3-enforcement-panel-2026-09-29.md`).
M3 · List MCP servers under Secrets “Used by” · branch `codex/m3-final-port` (independent MCP-only and mixed-use
checks passed. Pencil `wPXgW`/`R2ImH`/`RxpL1` and exact-PID app captures had no blocking or major mismatch after the
archive geometry fix. A bounded Add Secret → Staging → masked value → Archive flow passed a 944-sample host no-focus
watcher; earlier foreground episodes during taskbar switching remain documented. Seven named Secrets/i18n tests and
`tools/check.ps1` passed on the current-master port. Evidence: `docs/evidence/m3-secret-mcp-users-2026-09-29.md`).
D4 · Verify a UI change inside an Orchester run · branch `codex/d4-current-port` (DOS-12 passed
Implement checks, Build+Pin, independent read-only Review and Verify. The verifier compared saved New Task design
`m82tU` with the exact-PID app at 2160 × 1440 and found no blocking or major mismatch; the candidate's host-side
focus watcher passed at sampled instants. The host Orchester watcher recorded a separate unexplained foreground
episode. Implement rechecked a pre-authored UI candidate, so this does not establish self-hosted UI authoring.
Filtered New Task/saved-draft/pipeline tests and `tools/check.ps1` passed on the integrated port. Evidence:
`docs/evidence/d4-run12-2026-09-29.md`).
U5 · Align the remaining task UI states with Pencil · branch `codex/u5-current-port` (independent source-matched
verification passed 10 named tests and `tools/check.ps1`. Exact-PID app checks covered parallel waiting rows,
pipeline and environment details, and the transition from a queued message to *Not delivered* after Remove worktree.
The reached changed regions match saved Pencil frames `PacyY`/`Y8hyj` without blocking or major mismatches; a host
watcher recorded 2,461 samples without the test app taking focus. The live queued transition used a typed store seed;
the runner refusal was verified by a focused test, not a live Send. Illustrative design row counts differ from the
disposable fixture. Evidence: `.tmp/verify/u5-current-port-verify/report.md` and `.tmp/verify/u5/preflight.md`).
D7 · Keep embedded MCP bearer tokens out of agent transcripts · branch `codex/d7-safe-mcp-client` (the exact-PID
client reads discovery only in memory, rejects redirects and non-loopback URLs, and prints ASCII-safe redacted tool
results. Independent synthetic tests 9/9, `tools/check.ps1`, `git diff --check`, and a real Implement → Review run
passed. The run used D6 to pin its own app pair, called `lurq_windows` and `lurq_read_tree` by spawned PID without
printing the credential, then closed the app; a 200 ms host watcher recorded an unchanged foreground PID. Evidence:
`docs/evidence/d7-safe-mcp-client-2026-09-29.md`; usage: `docs/mcp-verification-client.md`).
D6 · Pin the verification app built by a run · branch `codex/d6-pin-verifier-app` (`orchester-exec` CLI copies the
desktop and runner to one run-owned directory and verifies source/copy SHA-256; independent synthetic tests 3/3,
`tools/check.ps1`, `git diff --check`, and macOS cross-check passed. Replacing the shared-target source after pinning
left both copies unchanged. The helper cannot prove which build produced source bytes before it opened them. Usage:
`docs/verification/pin-verifier-app.md`; review: `.tmp/verify/d6/report.md`).
D3 · First real Orchester runs on an isolated clone · branches `codex/d3-first-run` and `codex/d3-mcp-inside-run`
(Codex sandbox passed core check, two filtered runner process/pipe tests, desktop+runner MCP build and
`tools/check.ps1`; an in-run no-focus desktop PID answered exact-PID `lurq_windows` and `lurq_read_tree`, then closed;
separate Review passed. Sandbox foreground PID was unavailable, and follow-up host replays were inconclusive; D4
subsequently completed the UI verification setup. Evidence: `docs/evidence/d3-first-real-run-2026-09-29.md` and
`docs/evidence/d3-mcp-inside-run-2026-09-29.md`).
M2 · Map imported profiles' MCP servers · branch `codex/import-mcp` (source server IDs are resolved to target servers
or left explicit as unresolved; independent core import 20/20, store 1/1, desktop 1/1, i18n 2/2 and
`tools/check.ps1` passed. No-focus MCP/Pencil verification found 0 blocking/major/minor in the generic mapping row;
the imported draft uses the target server ID and the source database was unchanged. Pencil has no MCP-specific frame;
evidence `.tmp/verify/m2/report.md`).
U7 · Truncate long project names in sidebar selector and Overview heading · branch `codex/long-project-name`
(headless geometry test 1/1, `tools/check.ps1` passed; independent MCP/Pencil verification at 1440 and 900 logical px
found painted ellipses, preserved actions, and 0 blocking/major/minor in these regions. Evidence `.tmp/verify/u7/report.md`).
U9 · Truncate long project names in breadcrumb and Recent menu row · branch `codex/long-project-name`
(headless geometry test 1/1 including hover actions, `tools/check.ps1` passed; independent MCP/Pencil verification at
1440 and 900 logical px found painted ellipses, visible `Overview` crumb, separate menu actions, full stored name,
and no remaining long-name overflow on the screen. Evidence `.tmp/verify/u9/report.md`).
U4 · Publish button on a published profile · branch `codex/publish-button` (no code change: a published version has no
draft, so `Publish v2` is correctly disabled until edits are saved as draft v2. Real app lifecycle and MCP state
confirmed; focused lifecycle test 1/1 and `tools/check.ps1` passed. Pencil has no no-draft frame; the adjacent
saved-draft header mismatch is tracked as U8. Evidence `.tmp/verify/u3-u4/report.md`).
U1 · Truncate long status-bar failures · branch `codex/status-bar-truncation` (headless regression 3/3,
`tools/check.ps1` passed; independent MCP/Pencil verification at 1440, 1100 and 900 logical px found
0 blocking/major/minor, a painted ellipsis, full wrapped hint and no overlap with project counts;
evidence `.tmp/verify/u1/report.md`).
T1 · Drop waits the store retry made unnecessary · commit `c13ece9`.
T2 · Poll-free plan tests · commit `8739a65`.
D2 · Extra writable folders for agents · branch `claude/writable-roots`.
T3 · Serialize the remaining run request fallbacks · commit `389ee3b`.
T5 · Continuing a run whose worktree was deleted by hand · branch `codex/missing-run-worktree`.
L1 · The window title follows the active tab · branch `codex/window-title` (MCP and Pencil verified).
L2 · Remove the held scroll states · branch `codex/scroll-state` (15 offstage-only states removed; purposeful navigation and programmatic scroll states kept; `form_keyboard ::` 213 passed, 1 ignored; MCP and Pencil verified on reachable screens).
L4 · Verify text inputs after the text fixes · branch `codex/text-input-verification` (independent MCP/Pencil check of focused New Task brief, conversation composer, single- and multi-line selection, caret blink and stable line baseline: 0 blocking/major; 3 focused tests and `tools/check.ps1` passed. Minor: New Task form controls sit about 2–3 logical px above Pencil. Pencil has no explicit text-selection specimen; selection was checked against field geometry and the planned glyph-height behavior. Local evidence: `.tmp/verify/l4/report.md`).
L3 · Outside presses only close popups · branch `codex/popup-dismiss` (lurq 0.30.1 with upstream chrome fix; tab list, project switcher, permissions popover and selects checked through headless tests and MCP on page/chrome. Two regression tests; `form_keyboard ::` 215 passed, 1 ignored; `tools/check.ps1` passed).
L5 · Select menus wider than their trigger · branch `codex/select-menu-width` (lurq 0.30.2 adds `SelectStyle::min_menu_width`; the MCP secret menu is 260px wide and shows `NAME (environment)` on one line. Independent app/Pencil check: 0 blocking/major; registry-backed integration test 1 passed; `tools/check.ps1` passed. Minor fixture difference: stored references use canonical `@staging`, while Pencil shows `@Staging`. Local evidence: `.tmp/verify/l5/report.md`).
L6 · Verifier app windows start without focus · branch `codex/l6-registry-0303` (lurq 0.30.3 registry build and filtered focus-related tests 11/11 passed; `tools/check.ps1` passed. Independent exact-PID MCP and host foreground verification observed no focus across 951 samples; live Pencil Welcome comparison found 0 blocking/major, 1 pre-existing minor copy difference. Evidence: `docs/evidence/l6-registry-0303-2026-09-29.md`).
