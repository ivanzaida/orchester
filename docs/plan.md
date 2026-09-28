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
  own agent profile. Until a run can build and drive the app through its MCP server (see D3), leave UI tasks to work
  outside Orchester.
- When a task is done, move it to *Done* at the end of this file in the same change, with the commit or branch.
- Tasks marked **needs decision** wait for the user; do not start them.

## D · Developing Orchester inside Orchester

What is already in place: runs in their own worktrees, pipelines with agent and command steps, limits, questions and
the task conversation, Ready for review, *Merge into main* into the user's checkout, worktree cleanup, project
Environment variables, project MCP servers, tabs, and the installed stable build (`docs/stable-build.md`).

### D1 · Set up the Orchester project in the stable build (user, manual)

1. In the stable build, add `H:/projects/orchester` as a project.
2. Project instruction: *Read AGENTS.md at the repository root and follow it. Before writing Rust, read
   `.claude/skills/rust-code-quality/SKILL.md`. Take tasks from `docs/plan.md`.* Claude Code is started with
   `--setting-sources ""`, so it does not load `CLAUDE.md` on its own; Codex reads `AGENTS.md` itself.
3. Agent profiles: **Implement** (Codex, workspace write, network per the capability table) and **Review** (a different
   profile, read-only), so the reviewer is never the author.
4. Pipeline *Orchester change*:
   Implement → command `powershell -File tools/check.ps1` → command with the task's filtered `cargo test` lines →
   Review → Ready for review.
5. Environment: `CARGO_TARGET_DIR=<repo>/.orchester/target`; then, in Agents › Permissions of each profile that
   builds, add it to *Extra writable folders* (the tab offers it).
   Without it every run builds its own `target/` in its worktree (several GB, several minutes the first time).
6. Limits: start at about 3M tokens and 45 minutes of active agent time per run.
7. *Merge into main* merges into the user's checkout, which must be on `master` with no uncommitted changes. Do not
   merge from Orchester while another agent is merging branches into `master` by hand.

### D3 · Find what a run can and cannot do on this repository (first real run)

No run has been made on this repository yet. The first runs should be small non-UI tasks (for example T1 below). Record
what happens in `docs/evidence/` (dated file):

- whether `cargo build` / `cargo test` work inside Codex's Windows sandbox (`windows.sandbox=elevated`), including the
  runner tests that start processes and named pipes (`--features test-agent`);
- whether an agent can build and start the desktop with `--features mcp` and reach its MCP server, which a UI
  verification step needs;
- token use and time per step, and anything that failed or needed the user.

**Done when** the evidence file exists and every failure found has its own task in this plan.

### D4 · A UI verification step inside a run (after D3)

A pipeline step that verifies a UI change the way `.claude/skills/design-verification/SKILL.md` describes: Pencil
screenshots through the Pencil MCP server (configured as a project MCP server) and app screenshots through the app's
embedded MCP server. **Done when** a UI task runs end to end in Orchester with a verifier step whose report follows the
skill. Depends on D3's findings.

## T · Tests and runner

### T4 · A listing survives a row whose id cannot be parsed (non-UI)

A run row whose id or task id cannot be parsed still fails the whole listing
(`crates/store/src/project_db/runs/load.rs`), because it cannot say which run it is. Decide with the store owner
whether such a row is reported as a project-level "unreadable rows" error that still lists the others. **needs
decision**.

## M · MCP servers

### M1 · Confirm MCP behaviour in a real run (non-UI)

Unverified without a model turn: whether Codex under `-a never` lets the agent call an MCP tool, whether
`shell_environment_policy.exclude` keeps secret variables out of the agent's own commands, how Codex reports a server
that failed to start in a signed-in run, and whether Claude's `mcp__<name>` allow rule is enough. **Done when** each is
confirmed or fixed, with the observed CLI output saved as a fixture and parser tests updated
(`crates/core/src/connection/mcp/`).

### M2 · Import keeps a profile's MCP servers (non-UI + UI)

Importing an agent profile from another project keeps the source project's MCP server ids, so its agents work without
them. Map MCP servers in the import like connections (reuse or leave unresolved), following the import mapping rows
(`HfNoA`, `ixzlL`).

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

### U1 · Status bar labels never run under the right-hand group (UI)

A long failure label is not truncated and runs under the right-hand group of the status bar. Truncate with an ellipsis
and the full text in its hint.

### U2 · Merge and worktree controls while work runs (UI)

From the merge verification: *Remove worktree* can be pressed while a merge runs; *Cancel* stays enabled while a removal
runs; the *Merge* button gets narrower while merging; the Working branch row still names a branch that was deleted.

### U3 · Claude mechanism line in the enforcement panel (UI)

The Claude repository mechanism line wraps onto two lines in the 300 wide panel. Check the design's wording and width.

### U4 · Publish button on a published profile (UI, check first)

On the Reader profile the header showed *Published v1* next to a *Publish v2* button that looked unavailable. Find out
whether it is disabled and why, and whether that is right.

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

## L · After lurq 0.30.0

The fixes from lurq branch `claude/tabs-gaps` and the text-input fixes are released in 0.30.0. The workspace uses
0.30.0 since commit `71fd0b7`. The baseline `tools/check.ps1` and full `form_keyboard ::` passed on 2026-09-28
(212 passed, 1 ignored). L3 can proceed.

### L3 · Outside presses only close popups

lurq's popups, overlays and selects now consume the press that closes them (`OutsidePress::Consume` default,
`.outside_press(OutsidePress::PassThrough)` to opt out); tooltips with `dismiss_on_outside_click(false)` are unchanged.
No app change is expected. **Done when** the tab list (`tabs/strip/list.rs`), the project switcher, the permissions
popover and the selects are checked: a press outside closes them without activating what is underneath, and a
headless test covers the tab list case.

### L5 · Select menus wider than their trigger (lurq)

lurq makes a select menu exactly as wide as its trigger, so the MCP dialog's *Add a variable from a secret…* menu shows
each secret's environment on a detail line instead of *NAME (environment)* on one line (documented deviation). A lurq
option for a menu wider than its trigger would allow the drawn form. lurq change; the user releases lurq.

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

## Later

- **macOS:** build, install and smoke-test on a Mac (`docs/stable-build.md`, macOS section). All macOS code is only
  cross-compiled so far.
- **Project settings:** disk use of run worktrees.

## Done

T1 · Drop waits the store retry made unnecessary · commit `c13ece9`.
T2 · Poll-free plan tests · commit `8739a65`.
D2 · Extra writable folders for agents · branch `claude/writable-roots`.
T3 · Serialize the remaining run request fallbacks · commit `389ee3b`.
T5 · Continuing a run whose worktree was deleted by hand · branch `codex/missing-run-worktree`.
L1 · The window title follows the active tab · branch `codex/window-title` (MCP and Pencil verified).
L2 · Remove the held scroll states · branch `codex/scroll-state` (15 offstage-only states removed; purposeful navigation and programmatic scroll states kept; `form_keyboard ::` 213 passed, 1 ignored; MCP and Pencil verified on reachable screens).
L4 · Verify text inputs after the text fixes · branch `codex/text-input-verification` (independent MCP/Pencil check of focused New Task brief, conversation composer, single- and multi-line selection, caret blink and stable line baseline: 0 blocking/major; 3 focused tests and `tools/check.ps1` passed. Minor: New Task form controls sit about 2–3 logical px above Pencil. Pencil has no explicit text-selection specimen; selection was checked against field geometry and the planned glyph-height behavior. Local evidence: `.tmp/verify/l4/report.md`).
