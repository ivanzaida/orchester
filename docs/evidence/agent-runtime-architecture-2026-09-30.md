# Agent runtime permissions and build isolation

The stable reruns exposed shared runtime failures rather than task-specific defects. ORC-14
and ORC-45 reviewers could not run read-only file reads or `git diff`; ORC-39 could not request
approval for Pencil writes; ORC-17 and ORC-30 command checks resolved Cargo artifacts from a
different worktree revision. ORC-44's initial saved implementation was `5f11338` on
`orchester/orc-44-run-2`.

At the user's direction these fixes were implemented directly in the root checkout on
`master`, based on `37716df`, without a new commit. The ORC-44 checkpoint supplied the
effective-permission/App Server migration implementation. The root added Windows read-only
sandbox selection, worktree-scoped build directories, live verification and the popup layout fix.

## Resulting behavior

- Profile, pipeline and current-run effective permissions select Codex App Server when a
  command or MCP approval can be requested. The normal account and configured global MCP
  servers remain available. Other invocations retain `exec -a never`.
- Command approval returns exactly `accept` or `decline` for one action. It never selects
  a session grant. Additional permission overlays, file-change approval and unsupported
  requests fail explicitly. Stored decisions are tied to the invocation, thread and turn;
  stale or duplicate decisions cannot approve a later action.
- A waiting `exec` session can continue through App Server with the same session, run,
  worktree and uncommitted work. Published resources and completed steps remain intact.
- Windows read-only and write invocations both select the native elevated sandbox. The
  repository access mode remains separate; read-only access does not become write access.
- An absolute `CARGO_TARGET_DIR` is a build root. Agents and command tools receive
  `<root>/worktrees/<worktree-directory>`, including tool-level absolute overrides. Steps
  within one worktree reuse artifacts; parallel worktrees have separate directories. Stored
  project variables and relative targets retain their values. New targets require the cold
  10 GiB free-space reserve.
- Profile and run controls expose MCP approvals and command approvals. Long run permission
  popovers stay between the title bar and status bar and scroll in smaller windows.

## Behavioral verification

The normal signed-in native Codex CLI was updated from 0.159.0 to **0.159.2**. All published
Orchester agent profiles were moved to **gpt-6.1-sol** before the reruns. A local generated
CLI JSON schema and a `thread/start` probe confirmed the granular policy, user reviewer,
repository sandbox, writable roots and normal account contract. No model turn was used in
that contract probe. Native `codex sandbox` also executed a read-only `git diff --stat` with
`windows.sandbox=elevated`.

Initial verification used desktop and runner copied binaries and AppDb under
`.tmp/agent-runtime/app`. They opened disposable repositories only. The real project database
was not opened by that private build. The user subsequently authorized updating and restarting
the installed stable app.

Two real **gpt-6.1-sol** runs used the native Codex CLI and a disposable MCP write tool:

| Decision | Observed result |
| --- | --- |
| Approve once | The same turn resumed, the run succeeded and the fixed marker contained one line. |
| Deny | The step failed visibly and the marker still contained one line; no second write occurred. |

The marker server, invocation contract and captures are under `.tmp/agent-runtime/`.
Command approvals and migration from a waiting `exec` session were exercised with a protocol
fixture, separately from these native MCP runs. Three integration cases verified approved
command/MCP actions, denied command, and denied MCP write, while preserving the original
session/execution IDs, worktree, uncommitted file, pipeline snapshot and completed preparation
step. They also rejected stale and duplicate request IDs. These migration cases now include an
extra writable folder. A fourth case approves an action, stops the runner before its turn completes,
and verifies that reconnect does not start another invocation or replay the action.

The following named-filter test commands passed on Windows:

```text
cargo test -p orchester-core --test agent_invocation invocation_
cargo test -p orchester-core --test command_approvals approvals
cargo test -p orchester-core --test command_approvals permissions_asking
cargo test -p orchester-core --test permissions permissions_
cargo test -p orchester-store --test tool_approvals request
cargo test -p orchester-store --test tool_approvals claimed_approval
cargo test -p orchester-runner --features test-agent --test command_approvals command_approvals_
cargo test -p orchester-runner --features test-agent --test run_permissions run_permissions_
cargo test -p orchester-runner --features test-agent --test environment environment_
cargo test -p orchester-desktop --test form_keyboard agent_permissions_
cargo test -p orchester-desktop --test form_keyboard run_permissions_
cargo test -p orchester-desktop --test form_keyboard step_inspector_narrows_the_profile_commands_and_never_widens
cargo test -p orchester-desktop --test i18n every_key
cargo test -p orchester-desktop --test i18n locales_have
```

`powershell -File tools/check.ps1` passed after the final popup change: formatting, clippy
with warnings denied, source file sizes and UI literal checks. `git diff --check` passed.
`cargo build --release --locked -p orchester-desktop -p orchester-runner` also passed. The
paired binaries are copied to `.tmp/agent-runtime/release-candidate/`, with a source and
SHA-256 manifest. This directory contains no registered real projects or imported database.

## Independent visual verification

The separate `runtime_ui_verifier` compared app screenshots from the exact owned process
with live Pencil screenshots. The final popup build is process **41420**. Captures are under
`.tmp/agent-runtime/verify/`; canonical design group `DMBmo` is in the live Pencil document.
The user saves the design.

| Changed state | Pencil reference | Result |
| --- | --- | --- |
| Profile MCP control and enforcement explanation | `HnBBr`, `ossOP` | No blocking or major mismatch. |
| MCP approval modal | `h3KeVQ` | No blocking or major mismatch; about 2 px vertical difference. |
| Run permission popover | `W29Od2` | Final build has no blocking or major mismatch; title/status bars remain clear at 1440 × 1020 and 960 × 700 logical sizes, with reachable scrolling footer. |
| Pipeline approval required by profile | `wujuh` | No blocking or major mismatch in the changed control; clicking the locked-on switch does not change it or create a draft. |
| Command approval modal | `GbNSA` | No blocking or major mismatch; the longer fixture path adds one introductory line and natural height. |

Raw fixture data, the existing Skills sidebar, existing profile fields absent from older
frames, the popup's anchor position in the Summary route, and a dimmed profile-required
switch are documented state differences. The existing `Network stays on` wording versus
the design's `On` is minor. The earlier popup/status overlap was a major finding and was
fixed before the final recheck.

## Limits and remaining operation

Core and exec both passed `cargo check` and clippy with warnings denied for
`aarch64-apple-darwin`. The runner/desktop cross-check could not complete because this Windows
host lacks the target native C compiler for SQLite and zstd. No physical Mac test or full
test suite was run. Native command approval was subsequently exercised directly through App Server:
an exact-command temporary prompt rule produced a native `commandExecution/requestApproval` with
`environmentId: local`, no additional permissions, and sandbox approvals disabled. Approval wrote
one line; decline plus process termination left the marker at one line. The temporary rule was removed.
This native CLI probe is separate from the desktop protocol-fixture smoke below.

The protocol fixture was also driven through the desktop: **Approve once** created only
`command-once.txt`; the following MCP action was denied, leaving `mcp-once.txt` absent and
the run failed. Its persisted decisions are `approved` for the command and `denied` for the
MCP action. This UI smoke is a fixture test, not a native model command-approval result.

ORC-45 was created and started through the embedded MCP as run **13**. After the authorized stable
update, Review passed for ORC-14 and ORC-45; Check and Review passed for ORC-17 and ORC-30.
ORC-39's original native `exec` session resumed through App Server with the same ID and performed
approved Pencil writes. Saved worktrees and checkpoints remained present.

Resumed verifiers then exposed an App Server contract error: `environments: []` disables local
tools and the implicit writable worktree. Explicit local environments restore them. Codex reports
all configured runtime roots, including extra writable folders; validation checks that exact set,
and each turn receives it. Read-only startup overrides extra roots to empty. Native 0.159.2 probes
confirmed both access modes and the extra-root contract. A native model session was first run with
local access disabled, then resumed under the corrected local selection; it executed a shell write
successfully with the original session ID (`.tmp/agent-runtime/native-local/result.json`). A second
native resumed-session probe also wrote in the configured additional folder with sandbox approvals
disabled (`.tmp/agent-runtime/native-local-extra/result.json`).

The root also fixed ORC-44 Review findings directly: approvals do not allow sandbox exceptions,
known project MCP secrets are redacted before activity tracking/persistence (including JSON escapes),
and a soft stop after delivery records an interruption. The runtime-only fixes were transferred onto
ORC-44's saved work without changing the canonical design. Fresh downstream checks remain required.
Task-specific UI mismatches in ORC-30 were sent to its Implement agent for correction; the root
does not substitute its own QA for those agents. Final task verification remains in progress.

An existing production session was then resumed without starting a model turn. This exposed
Windows path normalization: Codex converts the runner's mixed separators to backslashes.
Environment cwd and root validation now compares filesystem paths, retaining exact root-set
equality instead of comparing their string spelling. The protocol fixture reproduces this
normalization; all four command approval regressions pass.

The next ORC-44 Review correctly returned `failed` for a separate Apply-now recovery gap.
The worker now waits for active MCP calls as well as shell commands. A permission restart
whose approval delivery lacks a confirmed result requests a checkpoint pause and requires
explicit continuation. Two runner regressions cover a held approved MCP action and a pending
decision interrupted for a permission change; both pass. The root implements these fixes
directly and transfers only the runtime delta into the preserved ORC-44 worktree.

The fresh ORC-30 Review exposed a separate result-contract ambiguity: its report contained a
negative verdict and two major defects, but its structured outcome was `done`. The shared
new/resumed prompt and result schema now define `done` as completion with passing required
checks. A completed Review/Verify with a negative verdict or unresolved blocking/major
findings must return `failed` so pipeline failure handling can route corrections. Genuine
missing decisions, access or permissions remain `question`. This preserves the existing
wire format and does not classify prose or rewrite past results. All three `outcome_prompt`
regressions pass; final `tools/check.ps1` is `check: OK`. Model compliance remains a live
agent responsibility, not something these contract tests guarantee.

## Installed contract and resumed task verification

The authorized stable update completed at 2026-09-30 15:44 UTC after owned desktop
57560 and runner 37456 exited through the save-and-close flow. The settled binaries and
both real databases were backed up under `.tmp/agent-runtime/pre-update-20260930-154420/`.
The install helper completed successfully; no second installer was started.

The installed pair in `E:/projects/orchester` matches the release candidate manifest:

| Binary | SHA-256 |
| --- | --- |
| `orchester.exe` | `5abac1de19516eeefd903bd10ce87d4a027c7a947e5b41e1c9fb3f05451aba3e` |
| `orchester-runner.exe` | `1e81790f2caddc5d065b291b6ad823260191c7d9dcba02c15c72237e157f02fa` |

The new owned desktop 63184 and runner 61600 opened the preserved project. The desktop
connected through exact-PID embedded MCP without taking focus. ORC-44's existing run
now has a 4,000,000-token budget; its 7,200,000 ms time limit is unchanged. Its fresh
Check and Review passed; independent Verify remains in progress. ORC-30 and ORC-39
resumed their existing Implement steps. ORC-14 and ORC-17 retain their completed
Implement work and await available slots for downstream verification.

ORC-45's independent Review and Verify pass within the approved scope; run 13 is
`succeeded`. Its `host-ready-155347` and `host-short-160636` timelines remain
inconclusive, with original PID markers, verdicts and gap summaries preserved.
The fresh `host-priority-161506` timeline passes the unchanged judge for owned
PID 30256 at a documented 1,000 ms interval. This covers sampled instants of that
short lifetime only, not continuous focus or previous functional lifetimes.
The verifier retained unreached-state and macOS limits in `.tmp/verify/report-final.md`.
ORC-14 resumed its existing Check in the released slot. No merge or root commit occurred.

## Native command decisions and recovery session validity

ORC-44's independent native verifier exposed a real 0.159.2 command request with
`availableDecisions: ["accept", "cancel"]`. Requiring `decline` rejected that request
before the approval dialog appeared. Root reproduced the envelope with one harmless
exact-command temporary rule; it did not approve or execute the command. The redacted
request is saved at `.tmp/agent-runtime/native-command-2bb73372/request.json`.
The earlier decline-plus-process-termination probe proved marker containment only;
it did not establish that `decline` was an advertised or accepted denial decision.

The parser now requires action-scoped `accept` plus an offered denial, preferring
`decline` when available and otherwise sending `cancel`. Legacy missing/null decision
lists retain `decline`; malformed, sticky-only and incomplete lists fail closed.
The chosen denial stays attached to the correlated request. No session-wide approval,
sandbox exception or policy amendment is introduced. Root also replaced the obsolete
single-run permissions notice with the exact current canonical Pencil guidance.
Only these three source files were transferred to ORC-44 after checking the original
bytes, without resetting its worktree. Independent task QA must cover the changed inputs.

Repeated downstream Verify recoveries also reused sessions containing a completed
negative assessment of an earlier candidate. Recovering an upstream step now starts
dependent provider sessions on the current source, including formerly failed verifiers.
Older failed parallel worktrees and invocation records remain intact, but are not reused
as the new candidate. Recovery-history walks stop at this invalidation boundary instead
of falling back to an older worktree or session. The selected step and unaffected
unfinished parallel steps keep their saved work and sessions.
Ordinary permission, question, budget and interrupted-step resumes are unchanged.
Every invocation's result contract now requires current-input provenance, fresh
reproduction of findings on changed work and explicit limits when reusing checks.

Validation of this additional root delta:

- Five filtered core `failed_recovery_` tests pass, including dependent-session
  invalidation, selected/parallel continuity and unchanged replayed history.
- Two runner `recovery_sessions_` regressions pass through real scheduler/storage,
  preserved uncommitted work and fake CLI arguments: Implement resumes, dependent
  Review starts fresh, old invocations remain intact and a fresh negative verdict stays failed.
  A formerly failed parallel Review receives the corrected upstream source in a new
  worktree; the old source and uncommitted partial work remain intact in its older worktree.
- All six existing runner `failed_recovery_` regressions and three `outcome_prompt_`
  regressions pass. The native command parser and locale regressions passed earlier
  in this same source milestone (three and four tests respectively).
- `tools/check.ps1` reports `check: OK`; core Darwin clippy with warnings denied passes.
  Full desktop/runner Darwin compilation and physical Mac testing remain unperformed
  for the native-toolchain limitation documented above.

The installed 15:44 pair remains the earlier contract; the new delta is not yet installed.
The six task runs retain their identities. ORC-45 is succeeded; current ORC-14/17/30
verification uses new candidates, ORC-39 needs continuation after the former daily
budget stop, and ORC-44 needs upstream adoption plus fresh independent QA. Its old
PID 20152 host verdict is inconclusive because of sampling gaps, and the verifier
found a real long-Target truncation without a hover reveal; that UI finding is pending
its task-specific Implement agent. The temporary project maximum and daily budgets
are 16M and 40M; default run/time limits remain 1.4M and 30 minutes. The user subsequently
authorized Root commits after inspecting the pending runtime changes. Task branches remain unmerged.

## Graceful shutdown while a tool decision is pending

The committed pair (`ad13563`, `c8c683f`) was installed at 18:50:48 UTC after its final
quality gate and release build. Actual installed desktop and runner hashes match
`.tmp/agent-runtime/current-runtime-candidate-v2/build-manifest.json`. That update did
not gracefully save ORC-14: the old app's CloseWaiting overlay covered its pending
Pencil approval and the run needed an explicit Quit now interruption and continuation.
Its existing session, checkpoint and worktree were preserved.

Root now fixes this deadlock in the runner instead of answering hidden approvals during
shutdown. Pause lets ordinary invocation work finish. When the invocation awaits a tool
decision, it ends and checkpoints the actual work without granting or denying that action.
The open execution and provider session are preserved. Interrupted approval records prevent
automatic replay when attaching again; continuation remains explicit. A decision received
after checkpoint termination starts is rejected.

The parser retains the correlated tool item so checkpoint safety excludes only that pending
item. Other active commands and MCP calls must finish first. A pause requested during an
approved command remains pending until it finishes; a later approval request then checkpoints
without executing the newly requested action. Apply-now uses the same safety check for a
pending decision alongside other active tools.

Validation of this additional, uncommitted root delta:

- Three `approval_shutdown_` scheduler/storage/protocol regressions pass: pending command,
  pending MCP, and active-command completion followed by a later pending MCP. They assert
  actual checkpoint contents, the preserved session/open execution, absence of unapproved
  side effects, and no automatic invocation after reconnect.
- The core `pending_approval_` test proves another MCP item remains active while one waits.
- Two existing `approval_permissions_`, four `command_approvals_`, one
  `lifecycle_finish_at_checkpoints_`, and one `control_pause_waits_` regressions pass.
- `tools/check.ps1` reports `check: OK` in `.tmp/agent-runtime/check-shutdown.log`;
  core Darwin clippy with warnings denied passes.

The shutdown correction has a successful release build and is ready for the Root commit the user
authorized on this turn. It is not installed. No task branch was merged. Physical Mac testing,
the full test suite, and full desktop/runner Darwin compilation
remain unperformed; the native C-toolchain limitation still applies.

## Immutable invocation launch permissions

ORC-44's independent Review found that a Next invocation change immediately appeared as the
active policy while the running agent still enforced its earlier permissions. Its Implement
agent identified a shared data gap: invocation records held no actual launch-policy snapshot,
and permission-change events record desired settings. Timestamp inference would invent evidence.

Root added the shared core/store/runner contract directly. `Invocation.permissions` is the
enforced `AgentPermissions` used by the prepared launch command. The runner records it once
before starting that process, including any network-on fallback when network-off cannot be
enforced. Later run overrides, progress saves, completion and session continuation never
rewrite an earlier invocation's snapshot. The store rejects another write, a command-tool
policy, a closed invocation and a missing invocation. A database trigger protects an existing
snapshot from direct update. The appended migration leaves older agent invocations explicitly
unknown; neither desired settings nor event timestamps backfill them.

Validation of this contract:

- Three `invocation_permissions_` store tests pass: immutable round-trip/reopen despite a
  conflicting progress record, invalid writes, and migration preserving the legacy session,
  invocation and run without inventing a policy.
- Six `run_permissions_` runner tests pass, including assertions that active policies remain
  unchanged until the next launch or Apply-now checkpoint, resumed sessions get the new
  snapshot, cancelled restarts retain their history, and Claude's unsupported network-off
  is recorded as the actual network-on policy.
- Four `command_approvals_` runner tests pass with assertions that migrating to App Server
  records the actual Ask/MCP policy and preserves preceding launch snapshots.
- Three `approval_shutdown_` tests pass with the new launch recording present.
- `tools/check.ps1` reports `check: OK` in
  `.tmp/agent-runtime/check-invocation-policy.log`; core Darwin clippy passes with warnings denied.

Root supplied the 14 API/source/test files to the preserved ORC-44 worktree after checking every
target. The only divergent target, `step/mod.rs`, retained its existing recovery behavior and
received only the two launch-recording lines. Original files and hashes remain in
`.tmp/agent-runtime/invocation-policy-handoff-v1`. Task-specific copy and evidence were preserved.
Its Implement agent owns the current-versus-next display correction and fresh independent QA.

The user authorized committing the current Root work and requires all subsequent Root source
changes in a separate worktree. The launch-policy delta is not installed yet. Physical Mac,
the full suite and full desktop/runner Darwin compilation remain unperformed.
