# ORC-7: implementation returned from Verify attempt 3

Candidate: base HEAD cb503ded89347f61dea8932e6c28808506ce4a8e plus the saved changes in this return. No commit was made. The previous report at .tmp/verify/orc7/report.md remains evidence for the old candidate only.

## Corrections

The Production-empty view now retains the table header and uses its compact 76-pixel empty row. Used by links use the design's 36-pixel raised surface. Archive confirmation has a raised footer, archive glyph and separate impact notice, with no extra value hint. Usage log has its divider, muted heading, three icon rows and scroll-text tile. The project-empty view starts directly below the tabs, has no environment/count row and includes the unavailable inner import action.

Also corrected singular counts, the empty/unknown tab labels, name placeholder, duplicate value hint, value-hint typography, replace header grouping and disabled-save reason alignment. Read failures retain the environment control and explain the failed read without claiming a zero count. Domain logic and authorization rules are unchanged.

Bundled-font mappings for the two new glyphs were corrected after native smoke inspection. The published Lucide mapping whose existing glyphs match the bundle is [lucide-static 0.560.0](https://app.unpkg.com/lucide-static%400.560.0/files/font/lucide.styl).

## Native QA fixtures for Verify

Source: crates/desktop-app/examples/secrets_qa/. Build with:

```
cargo build -p orchester-app --example secrets_qa
```

Launch the example with one argument: list, writes or empty. It starts at 1440 × 900 logical pixels, dark, without focus. On Windows launch with Start-Process -WindowStyle Hidden, a working directory under this worktree's .tmp/, and stdout/stderr redirected there. Copying the built executable to a separate .tmp/ directory avoids locking subsequent builds. The scratch launch helper used here is .tmp/orc7-return/launch.ps1. Do not use the person's installed app.

Every launch creates an isolated project/database/discovery directory under .tmp/qa/secrets-native/app-*/ and prints only its PID and discovery path. It uses an in-memory vault and no working runner. MCP references are illustrative; the stdio program uses the example executable with an invalid scenario argument that exits immediately, and HTTP hosts use .test. Close only this example's window/PID. A normal close removes its temporary directory.

- list: five metadata entries with mixed tool + MCP, MCP-only, tool-only and unused variants. Use the actual environment selector, rows and inspector actions. Usage log and Production-empty are reachable from here.
- writes: the same metadata without references, allowing native replacement pending/failure/retry through the normal workspace action without a person-only question.
- empty: no secrets, for the project-empty view and inner actions.

Use tools/lurq-mcp-client.py with the exact printed PID, --app orchester-app and --discovery-dir <printed path>. The example-only qa_secrets tool accepts {"action":"..."}:

| Action | Effect |
| --- | --- |
| fill | Supplies synthetic fixture input directly to the mounted masked field. No value is an argument or returned. Refuses person-only input and an absent/unmasked field. |
| hold | Holds subsequent vault store/delete calls (60-second timeout). Click the real save/replace/archive action to show pending. |
| deny | Releases the hold and makes vault writes/deletes return AccessDenied. |
| backend | Releases the hold and makes vault writes/deletes return Failed. |
| allow | Releases the hold and permits writes/deletes; retry with the actual screen action. |
| read-error | Corrupts only a known kind in this fixture's metadata and triggers the screen's normal refresh. |
| repair-read | Repairs the fixture metadata; use secrets-load-error-retry to exercise Try again. |

The tool reports only applied action and whether the vault entered an operation. It is registered only by this example, never by the production binary. The production MCP privacy rules and window input gate remain active. It cannot answer a person-only question.

For add states, open secrets-add, set a valid metadata name/environment, call fill then hold, and click secret-save. Call deny or backend for failure; allow and click secret-save for retry. Replacement uses secret-replace and the same controls in writes. Archive uses secret-archive to open confirmation, hold, then secret-archive again; deny/backend and allow expose failure and retry. Archive removes only fixture values and preserves archived metadata/references.

## States and Pencil nodes for independent Verify

Read only layer Secrets ccz8n in data/orchester2.pen through Pencil:

| State | Nodes |
| --- | --- |
| List and inspector | DrLE6 |
| Mixed, MCP-only, tool-only and unused references | LDP7i (including zAzUD) |
| New secret | YGy5i |
| Duplicate name and existing-secret action | Gam8g |
| Missing/invalid/too-long name and disabled reasons | j0ffR (including SJwDy) |
| Replace value | yTCmk |
| Archive confirmation, mixed uses | K2epbr |
| Archive variants, unused/MCP-only/tool-only | XoPMi (including esH3M, l1edUs) |
| Usage log | aKeYe |
| Project empty | L0uiU (TplaJ, GE9ao) |
| Production empty | Ds3OR (RlvMO) |
| List-read failure and retry | z9erZk (QOLZu) |
| Save/replace pending and failures | nykwz |
| Archive pending and failures | SsCvf |

Also check credential clearing, archived metadata, refreshed references and the in-place person-only replacement question/input lock/refusal/decline/retry. The list fixture reaches the question; it requires actual person input to authorize replacement. Automated coverage exercises both authorization and refusal. 960/light remain excluded; macOS native validation remains deferred to the person's Mac.

## Checks and commands

All commands were run from the worktree. No check.ps1 was run.

| Command | Outcome for final inputs |
| --- | --- |
| cargo test -p orchester-app --test secrets secrets_ | PASS, 18 tests (repeated after final source changes). |
| cargo test -p orchester-app --test screens screens_each_route | PASS, 1 test. |
| cargo test -p orchester-app --test ui_kit blocks:: | PASS, 13 tests. Shared EmptyState defaults are unchanged. |
| cargo test -p orchester-app --test ui_kit icons:: | PASS, 1 test. |
| cargo test -p orchester-app --test i18n locales_ | PASS, 3 tests. |
| cargo test -p orchester-app --test i18n an_area_file | PASS, 1 test. |
| cargo test -p orchester-app --test i18n every_key_ | PASS, 2 tests. |
| cargo clippy -p orchester-app --all-targets -- -D warnings | PASS after correcting intermediate compile/lint errors. |
| cargo +nightly fmt -p orchester-app | PASS; repeated during editing. |
| cargo +nightly fmt -p orchester-app -- --check | PASS. |
| cargo build -p orchester-app --example secrets_qa | PASS after correcting intermediate compile errors; repeated after final fixture correction. |
| cargo check -p orchester-app --all-targets | PASS at an intermediate layout revision; superseded by final all-target clippy. |
| cargo check -p orchester-app --example secrets_qa | Initial FAIL: draft/save errors needed explicit conversion to the example's error type; corrected and final build/clippy pass. |
| git status --short | PASS; scoped app source/tests, QA example and this evidence file; no commit. |
| git diff --stat | PASS; inspected scoped changes. |
| git diff --numstat | PASS; inspected final tracked change counts. |
| git diff --check | PASS. |

Intermediate failures: example build needed the same error conversions; later build/clippy found a missed Option wrapping at one filters call, corrected; clippy found println! in the QA launcher, replaced by fallible writeln!. The final fixture attempt using an absent stdio executable also failed the QA seed regression (17 passed, 1 failed); the workspace correctly rejected it. Replaced by an existing executable with an immediately exiting argument. These failures are superseded by passing final checks.

Native commands used .tmp/orc7-return/launch.ps1 and smoke.py (scratch only). Initial python tools/lurq-mcp-client.py 4032 lurq_windows --app orchester-app --discovery-dir .tmp/qa/secrets-native/app-kaIgM0/discovery failed with PermissionError in the sandbox; the same command passed with granted escalation. The native window reported focused=false.

Launches: powershell -NoProfile -File .tmp/orc7-return/launch.ps1 list, writes, empty — PASS. Initial list PID 4032; writes PID 30248; empty PID 2860. Exact-PID smoke commands:

- python .tmp/orc7-return/smoke.py 4032 .tmp/qa/secrets-native/app-kaIgM0/discovery list — captured list, usage, Production-empty, archive confirmation, read failure; initial script FAIL on incorrect retry control ID. This was a scratch script defect, corrected.
- python .tmp/orc7-return/smoke.py 4032 .tmp/qa/secrets-native/app-kaIgM0/discovery retry — PASS: read repaired and actual retry action recovered.
- python .tmp/orc7-return/smoke.py 4032 .tmp/qa/secrets-native/app-kaIgM0/discovery close — PASS.
- python .tmp/orc7-return/smoke.py 30248 .tmp/qa/secrets-native/app-zFlrW2/discovery writes — PASS: add pending, refusal, retry and save.
- python .tmp/orc7-return/smoke.py 30248 .tmp/qa/secrets-native/app-zFlrW2/discovery lifecycle — PASS: replace pending/backend failure/retry; archive pending/refusal/retry.
- python .tmp/orc7-return/smoke.py 2860 .tmp/qa/secrets-native/app-DVajfR/discovery empty — PASS.
- powershell -NoProfile -File .tmp/orc7-return/launch.ps1 list — PASS again for the final rebuilt fixture, PID 32832.
- python .tmp/orc7-return/smoke.py 32832 .tmp/qa/secrets-native/app-UVfBDq/discovery list — PASS: fresh list, Usage log, Production-empty, archive confirmation, unknown-count read error and retry recovery captures.
- python .tmp/orc7-return/smoke.py 32832 .tmp/qa/secrets-native/app-UVfBDq/discovery close — PASS.
- python .tmp/orc7-return/smoke.py 30248 .tmp/qa/secrets-native/app-zFlrW2/discovery close — PASS.
- python .tmp/orc7-return/smoke.py 2860 .tmp/qa/secrets-native/app-DVajfR/discovery close — PASS.

All four QA processes were closed through their own exact-PID MCP windows. No installed app process was touched.

Captures are under .tmp/orc7-return/app/. List/Usage/Production/archive/read-error captures are from the final rebuilt fixture (PID 32832); earlier intermediate list smoke captures were superseded. Writes/empty captures include all UI corrections; the final fixture-only edit adds an immediately exiting argument to the illustrative stdio program and leaves their UI inputs unchanged. The original Verify report and paired captures under .tmp/verify/orc7/ are preserved for the prior candidate. Independent Verify must use the final rebuilt fixture and fresh Pencil exports; this report makes no independent visual-parity claim.

Not run: check.ps1 (pipeline Check is authoritative), unfiltered/full-workspace tests, workspace filtered tests in this return (workspace source unchanged), macOS native validation, packaging/installation, OS keychain integration, clipboard/native person confirmation, 960/light and independent design verification. docs/plan.md remains pending until Check and Verify pass. No writes to the deprecated app, the design file or the person's install.
