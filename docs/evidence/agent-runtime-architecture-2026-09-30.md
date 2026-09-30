# Agent runtime permissions and build isolation

The stable reruns exposed shared runtime failures rather than task-specific defects. ORC-14
and ORC-45 reviewers could not run read-only file reads or `git diff`; ORC-39 could not request
approval for Pencil writes; ORC-17 and ORC-30 command checks resolved Cargo artifacts from a
different worktree revision. ORC-44's saved implementation remains at `5f11338` on
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

The updated desktop and runner ran from their own copied binaries and AppDb under
`.tmp/agent-runtime/app`. They opened disposable repositories only. The real project database
was not opened by this build, and the installed stable binaries were not replaced or stopped.

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
step. They also rejected stale and duplicate request IDs.

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
test suite was run. The command-approval smoke and exec-to-App-Server migration used protocol
fixtures; no real model command approval or real model migration is claimed.

The protocol fixture was also driven through the desktop: **Approve once** created only
`command-once.txt`; the following MCP action was denied, leaving `mcp-once.txt` absent and
the run failed. Its persisted decisions are `approved` for the command and `denied` for the
MCP action. This UI smoke is a fixture test, not a native model command-approval result.

ORC-45 was created and started through the stable app's embedded MCP as run **13**. Its
Implement and Check passed, while Review waits on the old stable read-only sandbox defect.
ORC-14 and ORC-39 remain waiting, ORC-17 and ORC-30 failed at Check, and ORC-44 remains paused
at its time limit. Their saved work has not been removed. Applying this runtime to the
installed stable app and continuing those tasks requires a separately authorized installation
and restart, per the repository's stable-build rule.

## Resumed ORC-44 implementation

The user subsequently reported that the root completed the installation and applied the remaining
runtime/UI/Cargo delta to `orchester/orc-44-run-2`. The Implement agent inspected checkpoint
`9b3484f`, its saved `5f11338` implementation, that applied delta, and the independent verifier's
root `.tmp/agent-runtime/verify/report.md`. The current effective-policy and migration code already
incorporates the earlier work; replaying the earlier patch would replace newer behavior.

This resumed step preserves the applied delta and performs no installation, stable-process control,
design writes, or implementer design verification. The original installation-pending note above is
historical. The user's later installation report does not turn fixture command/migration coverage
into native model evidence.

The later independent steps must check native harmless command approve/deny, native migration from
a pre-existing noninteractive session, one-action/duplicate/session scoping, interruption recovery,
paused active-time accounting, unchanged published/other-run policies, and inherited MCP/account
availability. Design comparison uses the already-authored `DMBmo`, `W29Od2`, `HnBBr`, `ossOP`,
`h3KeVQ`, and `GbNSA` references at matching app sizes and exact owned PID, without interacting
with the stable app. Physical Mac verification and full runner/desktop cross-check remain unproven.

### Final checks in the resumed worktree

All **67 filtered tests passed** in this worktree's isolated Cargo target directory:

| Crate / test binary | Named filters | Passed |
| --- | --- | --- |
| core / agent_invocation | `invocation_` | 11 |
| core / command_approvals | `approvals`, `permissions_asking` | 3 |
| core / permissions | `permissions_` | 16 |
| core / run | `run_permissions` | 8 |
| store / tool_approvals | `request`, `claimed_approval` | 4 |
| runner / command_approvals | `command_approvals_` | 3 |
| runner / run_permissions | `run_permissions_` | 6 |
| runner / environment | `environment_` | 6 |
| desktop / form_keyboard | `agent_permissions_`, `run_permissions_`, `step_inspector_narrows_the_profile_commands_and_never_widens` | 8 |
| desktop / i18n | `every_key`, `locales_have` | 2 |

Each row used `cargo test -p orchester-<crate> --test <binary> <filter>`; runner rows also
used `--features test-agent`. Final `powershell -File tools/check.ps1` returned **`check: OK`**
(formatting, workspace/all-targets/all-features clippy with warnings denied, file sizes, UI literals).
`git diff --check` passed. Current core/exec `cargo check` and clippy with `-D warnings` both
passed for `aarch64-apple-darwin`.

The resumed implementer did not run the full test suite, rebuild/install a release, perform
native model smoke tests, or perform design verification. Native MCP smoke and independent
visual results above belong to the root's prior verification. Native command/session migration
coverage and physical Mac runtime verification remain explicitly for independent follow-up.
