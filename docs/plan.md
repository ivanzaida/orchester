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
5. Environment: `CARGO_TARGET_DIR=<repo>/.orchester/target`, and allow agents to write to it once D2 is merged.
   Without it every run builds its own `target/` in its worktree (several GB, several minutes the first time).
6. Limits: start at about 3M tokens and 45 minutes of active agent time per run.
7. *Merge into main* merges into the user's checkout, which must be on `master` with no uncommitted changes. Do not
   merge from Orchester while another agent is merging branches into `master` by hand.

### D2 · Extra writable folders for agents (in progress, branch `claude/writable-roots`)

Codex runs with `--sandbox workspace-write -C <worktree>`, so it cannot write to a shared `CARGO_TARGET_DIR` outside
the worktree. The branch adds a visible *extra writable folders* setting and passes it to the CLIs.

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

### T1 · Drop waits the store retry made unnecessary (non-UI, good first Orchester run)

The desktop `form_keyboard` lifecycle tests wait for a killed runner to fully end before reading its database. Since the
store retries a read while a crashed runner's shared memory is still mapped (`crates/store/src/migrate.rs`,
`connect_with`), that wait may no longer be needed.
**Done when** each such wait is either removed with the test still passing 5 times in a row under
`--test-threads=16`, or kept with a comment saying why. Tests: `cargo test -p orchester-desktop --test form_keyboard
lifecycle`, then the full `form_keyboard ::`.

### T3 · Serialize the remaining run request fallbacks (non-UI)

`continue_runs` and reconnect continuation are synchronous and do not take the continue lock; the pause, cancel and
permission fallbacks are not serialized with a concurrent request that starts a task for the same run
(`crates/runner/src/execution/`). A clash now comes back as an explicit `Failed`, not silently. **Done when** every
request that can start or stop a run's task takes the same lock, with a test per path. Tests: runner `control`,
`settling`, `plan_settling`, `lifecycle`.

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

### U6 · Conversation states not seen in the app (UI)

The Paused, Stopped, Interrupted and Not started states of the conversation panel, and direct messages to one agent,
are covered only by data tests. Verify them in the app and fix what differs from `jgqPe` / `kjsmd`.

## L · Depends on a lurq release

In progress on lurq branch `claude/tabs-gaps` (the user releases lurq). After the release that contains them:

### L1 · The window title follows the active tab

Use lurq's new runtime window title API so the OS title names the active tab's project and page.

### L2 · Remove the held scroll states

Offstage tabs did not keep scroll positions, so about 29 page components hold their own `ScrollState`
(`crates/desktop/src/screens/`). Once lurq keeps runtime state across offstage, remove them and keep the tab tests
green (`form_keyboard tabs`).

### L3 · Outside clicks only close popups

After lurq stops passing the outside click through, check the tab list, selects and popovers, and remove any app-side
workaround.

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

Move finished tasks here: `ID · title · commit or branch`.

T2 · Poll-free plan tests · branch `codex/poll-free-plan-tests`.
