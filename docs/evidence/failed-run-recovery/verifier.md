# Independent verification: failed-run continuation

Date: 2026-09-30 (Europe/Kyiv). Candidate: `H:\projects\orchester\.tmp\worktrees\codex-current-integration`, branch `codex/continue-failed-run` (base `6626cc4` plus current uncommitted change). Verification only; I edited no candidate source.

## Verdict

No blocking or major finding in the changed Failed-run conversation path. The retained-worktree `Send & continue` state and the removed-worktree closed state are reached in a disposable project. The former matches the current main Pencil frame `vUPqr` in its changed notice, status, composer, action, explanation, split, and spacing. The runner tests use a fake Codex CLI and prove same-run/session recovery and durable history for the tested cases; they do not prove a live model's behavior. A physical Mac was unavailable.

## Automated checks

Commands ran from the integration worktree with its existing `target` directory:

| Command/filter | Final result |
| --- | ---: |
| `cargo test -p orchester-runner --features test-agent --test failed_recovery failed_recovery --locked` | 6/6 |
| `cargo test -p orchester-core --test run failed_recovery --locked` | 3/3 |
| `cargo test -p orchester-runner --features test-agent --test conversation conversation --locked` | 13/13 |
| `cargo test -p orchester-desktop --test conversation conversation --locked` | 9/9 |
| `cargo test -p orchester-desktop --test form_keyboard conversation --locked` | 5/5 |
| `cargo test -p orchester-store --test conversation conversation_recovery --locked` | 2/2, including migration |
| `cargo test -p orchester-core --test run outcome_prompt --locked` | 3/3 |
| `cargo test -p orchester-desktop --test i18n locale --locked` | 4/4 |
| `cargo test -p orchester-desktop --test i18n every_key --locked` | 1/1 |
| `powershell -File tools/check.ps1` | `check: OK` |

Thus 46 filtered tests passed on the final candidate. The first run of the new headless failed-run test hung because it waited for the successful-run notice; the implementer fixed that test predicate, and its final filter passed 5/5. A first fake-CLI stale-message assertion failed because its fixture prompt did not trigger a failure; the implementer corrected the fixture, and final failed-recovery filter passed 6/6. No production behavior was changed to silence those test issues.

`cargo check -p orchester-core --target aarch64-apple-darwin --locked` passed. The combined core/store/runner cross-check was attempted and stopped in `libsqlite3-sys`: this Windows host has no target `cc` compiler (`ToolNotFound`). Store and runner did not complete the macOS cross-check. No Mac runtime check was possible.

## Live app and Pencil comparison

- Disposable fixture: `H:\projects\orchester\.tmp\verify\failed-recovery\repo`; seeder: `seeder/src/main.rs`. The app and runner were copied to `bin` with their own `orchester.db`. The real runner invoked `orchester-fake-codex.exe` and produced a Failed run with an actual retained run worktree and checkpoint. It did not invoke a live Codex model.
- Own app PID **49380**, runner PID **36960**. Started with `ORCHESTER_MCP_START_WITHOUT_FOCUS=1` and `Start-Process -WindowStyle Hidden`, then positioned only the own HWND with `SWP_NOACTIVATE` on non-primary monitor `[2560,0,5120,1440]`. `lurq_windows` reported `focused:false`. Host foreground PID was **10132** before positioning, after positioning, after MCP interactions, and after close. The stable `E:\projects\orchester` install and its processes were untouched.
- Embedded MCP was addressed by PID **49380** for `lurq_windows`, `lurq_read_tree`, `lurq_interact`, and `lurq_screenshot`. The app screenshot was `2160×1530` at scale 1.5 for a 1440×1020 logical window. [Full app screenshot](app-full.png) includes 32px logical custom title bar and 28px status bar. [Normalized content screenshot](app.png) is 1440×960 after deterministic 2/3 scaling and cropping those bars, matching the [live Pencil `vUPqr` export](design.png) at 1440×960. The `.pen` file was accessed only through Pencil MCP; no Pencil document was switched or edited by this verifier.

| Region | Result |
| --- | --- |
| Content frame, sidebar boundary, top task header, tabs, conversation split | Match at the same logical 1440×960 size. |
| Failed notice | Icon, surface, two lines, padding and position match. |
| Conversation header/status/recipient | Status badge and control geometry match. Fixture has one Builder, while Pencil's illustrative data has four profiles. |
| Composer | Placeholder, Attach state, disabled `Send & continue` button, button size and hint copy match. |
| Worktree context | Present with real branch/checkpoint; fixture values and single failed step differ from Pencil's five illustrative steps. |
| Topbar | `Remove worktree`, `Run again`, `Edit` match the refreshed design frame. |
| Existing frame differences | This fixture has no prior conversation messages, whereas Pencil shows three illustrative messages; it has one pipeline step, so the left context begins higher. Sidebar `Skills` and Orchestrator's lock icon are pre-existing shared UI/design differences outside this change. |

After capturing the retained-worktree state, I used the own app's MCP confirmation to remove only that disposable run worktree. The Failed badge remained, the composer became closed with `No agent of this run reads messages any more.`, Send stayed disabled, and the explanation named the removed worktree. [Closed-state screenshot](worktree-removed.png). There is no new Pencil frame for this closed state, so the live comparison above applies to the retained-worktree state. The command-only `FailedNoAgent` state was not live exercised; its code guard was reviewed, and the parent chose to keep verification bounded.

I sent `lurq_interact request_close` only to PID 49380. Both own app PID 49380 and runner PID 36960 had exited afterward. The foreground remained PID 10132. No global input or whole-screen capture was used.

The current Pencil frame was exported while main `data/orchester.pen` was open; user Save for the latest design edits is still pending at this report time. The exported evidence records exactly what was compared.
