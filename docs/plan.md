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
  own agent profile. Until D4 provides an independent *Verify* step with Pencil and app screenshots and a host-side
  focus check, leave UI tasks to work outside Orchester.
- When a task is done, move it to *Done* at the end of this file in the same change, with the commit or branch.
- Tasks marked **needs decision** wait for the user; do not start them.

## Work in progress · 2026-09-29

| Task | Branch | Verified progress and next gate |
| --- | --- | --- |
| L6 | `codex/verifier-no-focus` | lurq 0.30.3 candidate and no-focus app checks pass; release approval is pending. |
| U2 | `codex/worktree-controls` | Real merge/removal and independent tests pass; Pencil removal, merging and deleted-branch states need correction and another design check. |
| U3 | `codex/claude-mechanism` | Live Pencil/app comparison passes; the edited `.pen` is not saved into the worktree yet. |
| M3 | `codex/secret-mcp-uses` | Independent mixed-use tests and MCP-only live checks pass; the live mixed state and exact MCP/mixed *Used by* Pencil frames are unavailable. |
| M5 | `codex/mcp-reached-by` | Independent tests and live checks pass; an exact remote-selected inspector Pencil frame is missing. |
| U5 | `codex/u5-small-mismatches` | Independent tests and live checks, including real parallel waiting rows, pass; the after-Remove-worktree live state was not reached and its Pencil frame is missing. |
| U6 | `codex/u6-conversation-states` | Paused/Stopped and direct sends pass independent checks; exact Not started, Interrupted and recipient-menu Pencil frames are missing. |

M1 evidence and parser tests are on `master`; Claude's MCP allow rule still needs a signed-in model turn.

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

### D5 · Check free disk space outside the agent CLI (non-UI)

In the first D3 run, Codex 0.139.0 with `-a never` declined `Get-PSDrive`, `fsutil volume diskfree H:` and `Get-Volume H`
in a read-only step. A task that required that check stopped before cargo. Add an Orchester host-side free-space
preflight for builds using a shared target, with a clear low-space result on Windows and macOS, so the agent does not
depend on shell commands that its sandbox may decline. Evidence: `docs/evidence/d3-first-real-run-2026-09-29.md`.

### D4 · A UI verification step inside a run (after D3)

A pipeline step that verifies a UI change the way `.claude/skills/design-verification/SKILL.md` describes: Pencil
screenshots through the Pencil MCP server (configured as a project MCP server) and app screenshots through the app's
embedded MCP server. Use L6's no-focus startup, a writable run-owned discovery data directory for both app and client
(`%LOCALAPPDATA%` on Windows; `$HOME/Library/Application Support` on macOS), an exact-PID client that does not print
discovery credentials (D7), and pinned app artifacts (D6). Inside Codex's Windows
sandbox `GetForegroundWindow` returned PID 0; verify host foreground separately instead of treating `0 -> 0` as
evidence that the user's window stayed active. **Done when** a UI task runs end to end in Orchester with a verifier step
whose report follows the skill. D6 and D7 are complete; the L6 release remains a prerequisite.

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

**Status 2026-09-29:** signed-in Codex 0.139 model turns confirmed MCP calls under `-a never`, a test variable present
in the server but absent from an agent shell command, and an unnamed stderr warning for a failed server while the turn
completes. One initial unannotated call was cancelled; two later unannotated calls succeeded, so the cause is unknown.
Claude 2.1.89 listed `mcp__m1probe__ping` with the allow rule but stopped at `authentication_failed` before a model turn.
Keep M1 open until that rule is checked in a signed-in Claude run. Evidence: `docs/evidence/mcp-real-run-2026-09-29.md`.

### M3 · Secrets "Used by" lists MCP servers (UI)

A secret used by an MCP server variable or bearer token is not listed under its *Used by*.

### M5 · Remote server details name who reaches it (UI)

The MCP server inspector (`crates/desktop/src/screens/project/connections/mcp_inspector.rs`) shows only Type, Endpoint
and Authentication for a remote (HTTP) server; 04.D `QdLRd` suggests a *Reached by* row as the dialog has
(`connections.mcp.property.reachedBy`). Confirm with the drawing, then add it. Also from that verification: the
Arguments and Variables text in the MCP dialog sit about 4px higher than drawn, and the 04.D details rows 2–4px lower.

### M6 · Environment names in secret references (needs decision)

Environments display in lower case (`@staging`, *(staging)*) where the design writes *Staging*, and typing `@Staging`
is refused (`Environment::as_str` in core). Decide whether names are case-insensitive with a display form; ties in with
the Secrets decision under P.

### M4 · Not built yet (needs decision)

Tool discovery and OAuth sign-in for MCP servers, narrowing servers per pipeline step, changing a running run's servers,
and MCP tools in the Tools library.

## U · Screens

### U2 · Merge and worktree controls while work runs (UI)

From the merge verification: *Remove worktree* can be pressed while a merge runs; *Cancel* stays enabled while a removal
runs; the *Merge* button gets narrower while merging; the Working branch row still names a branch that was deleted.

### U3 · Claude mechanism line in the enforcement panel (UI)

The Claude repository mechanism line wraps onto two lines in the 300 wide panel. Check the design's wording and width.

### U5 · Smaller mismatches left by earlier verifications (UI)

- Pipeline editor: *Back to editing* label is larger than 11/500; the canvas has no dot grid; the *Changed in draft*
  tag's icon is slightly large and its border faint.
- Conversation: *change ↗* uses the heavier link style; the Permissions row is 39 below the row above it, not 42; the
  composer is about 3px taller; waiting parallel steps are grouped into one row where the design shows two.
- Environment tab: the text area is 100 high where the design node is 116; *Add CARGO_TARGET_DIR* is centred instead
  of top-aligned.
- Tabs: the *+* button sits about 2px left of the design.
- All projects: headings sit about 2px high.
- After Remove worktree: the Summary card and the Conversation notice do not say the worktree was removed; the
  explanation wraps to two lines; the permissions row still shows *view ↗*. The state is not drawn yet
  (`docs/design/orchester-workspace.md`, "After Remove worktree").

### U6 · Conversation states not seen in the app (UI)

The Paused, Stopped, Interrupted and Not started states of the conversation panel, and direct messages to one agent,
are covered only by data tests. Verify them in the app and fix what differs from `jgqPe` / `kjsmd`.

### U8 · Agent profile header states need a design check (UI/design)

The published profile with no draft has no Pencil frame. In the saved-draft frame `WPmtS`, the app also shows a
disabled *Save draft* button that the drawing omits (major visual mismatch). Draw the missing state, decide whether
the disabled button should be shown, then align the app and verify both states. Evidence: `.tmp/verify/u3-u4/report.md`.

## L · After lurq 0.30.0

The tab and text-input fixes shipped in lurq 0.30.0, the outside-press fix for window chrome in 0.30.1, and the
select-menu width option in 0.30.2. This workspace now uses 0.30.2.

### L6 · Verifier app windows take focus (lurq or test tooling)

A verifier's own app instance received real keyboard input while the user worked (extra tabs opened, text typed into
a field), probably because the window took focus when it opened. Find a way to start agent-driven app instances without
activating them (a lurq window option such as "start without focus", or a flag the MCP build honours), so they never
catch the user's typing.

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
tracks the remaining UI verification setup. Evidence: `docs/evidence/d3-first-real-run-2026-09-29.md` and
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
