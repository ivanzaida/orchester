# ORC-3 implementation handoff — 2026-10-10

Candidate: saved, uncommitted work on `orchester/orc-3-run-1`, based on
`c263258d145eb22c167bda6a702e6e2cf35a6615`, in run worktree
`01a12776-6863-771a-ac12-62e125bd1e06`. No install data or installed process was touched.
This is implementation evidence, not an independent design-verification verdict.

## Implemented

- Pipelines library, empty/loading/error states, state/version/flow columns and virtualized rows.
- New pipeline is a page: starter, empty or copy of a published pipeline; duplicate slugs fail in place.
- Editor: agent/tool assignments, model, instructions, dependencies, failure routes/retries, budgets,
  approval gates and supported profile permission narrowing. Invalid fields survive changing the selected step.
- Valid edits autosave after a short debounce; Save draft also saves explicitly. Widening draft changes
  require protected in-place confirmation. Published versions remain immutable; history can supply a new draft.
- Publish reviews problems and changes, then asks the person for first publication or a weaker version.
  The Connections confirmation component provides the window-input lock.
- Workspace owns creation, narrowing, widening decisions and writes. Domain MCP tools share the extracted
  narrowing/widening rules. Transactional matching prevents stale saves and pins the version used by publication review.
- Writes notify project feeds. Router registrations and headless tests include the new pages.
- Settings is a page with editable description and resource identity. Design note D0hfv calls run defaults,
  failure defaults, permission ceilings and archive/delete proposals “not in spec yet”. These remain visibly
  unavailable; implementing new runner/settings semantics was kept outside ORC-3's acceptance criteria.

## Commands and outcomes

All test invocations had explicit name filters. Test scratch locations were directed into
`.tmp/pipelines/tests` through process-local TMP/TEMP environment variables.

| Command | Outcome |
| --- | --- |
| `cargo test -p orchester-app --test pipelines pipelines_` | PASS, 6 tests on the final implementation; earlier iterations exposed fixture/readiness problems which were corrected |
| `cargo test -p orchester-app --test routes every_` | PASS, 3 tests; initially exposed missing editor/creation router registrations, now fixed |
| `cargo test -p orchester-app --test routes pipelines_` | PASS, 1 test |
| `cargo test -p orchester-app --test screens screens_` | PASS, 2 tests |
| `cargo test -p orchester-app --test i18n every_key_named_in_code_has_its_area_file_and_exists` | PASS, 1 test; a workspace error-key comparison was moved out of UI after the initial failure |
| `cargo test -p orchester-app --test i18n locales_` | PASS, 3 tests |
| `cargo test -p orchester-app --test ui_kit table` | PASS, 1 test; existing table defaults preserved |
| `cargo test -p orchester-app --test mcp_domain domain_pipeline` | PASS, 3 tests |
| `cargo test -p orchester-app --test mcp_domain domain_automation_publishes_only_a_pipeline_no_weaker_than_the_last` | PASS, 1 test |
| `cargo test -p orchester-app --test mcp_domain domain_dropping_a_steps_approval_needs_a_person` | PASS, 1 test |
| `cargo test -p orchester-app --test mcp_domain domain_window_input_cannot_` | PASS, 3 tests |
| `cargo test -p orchester-workspace --test pipeline_writes pipeline_writes` | PASS, 1 test |
| `cargo test -p orchester-workspace --test i18n locales_` | PASS, 3 tests |
| `cargo test -p orchester-store --test draft_matching pipeline_draft_matching` | PASS, 1 test |
| `cargo test -p orchester-store --test resources resources_` | PASS, 14 tests |
| `cargo test -p orchester-store --test resources a_draft_is_published_only_while_it_is_the_one_checked` | PASS, 1 test |
| `cargo check -p orchester-workspace` | PASS during implementation; final workspace compilation covered by check.ps1 |
| `cargo check -p orchester-app` | PASS during implementation after intermediate compile fixes; final compilation covered by check.ps1 |
| `cargo clippy -p orchester-app -p orchester-workspace -p orchester-store --all-targets --all-features -- -D warnings` | Intermediate failures corrected; final broader clippy in check.ps1 passes |
| `cargo fmt --all` | Failed: Windows command-line length limit (os error 206) |
| `cargo fmt -p orchester-app -p orchester-workspace -p orchester-store` | PASS, with stable-toolchain warnings about nightly options; superseded by nightly formatting |
| `cargo +nightly fmt -p orchester-app -p orchester-workspace -p orchester-store` | PASS |
| `powershell -File tools/check.ps1` | PASS on final Rust inputs. Earlier failures were Windows rustfmt argument length and corrected compile/clippy issues |
| `cargo check -p orchester-app --target aarch64-apple-darwin` | Could not compile native dependencies: Apple C toolchain unavailable (`cc` not found) |
| `git diff --check` | PASS |
| `git status --short`, `git diff --stat`, `git rev-parse HEAD`, `git branch --show-current` | PASS, inspection only |

`tools/check.ps1` now formats one workspace package at a time. This checks every workspace member while
avoiding Windows' command-line length limit in the long run-worktree path. Its remaining checks are unchanged.

Not run: unfiltered/full cargo test suites, installed-app tests, graphical end-to-end tests, macOS runtime
tests, a completed macOS app build, the 960/light passes, or independent screenshot verification.
Earlier failures are historical observations, not verdicts on the final candidate. Tests of unchanged store,
workspace rules and UI-kit behavior were reused after unrelated UI refactors; the final check covers all Rust inputs.

## Verify handoff

Read the user's original design only through Pencil, without mutations. Compare dark desktop states in
`data/orchester2.pen`, layers Pipelines `i4jt6V` and Pipeline settings `vEDYY`:

| State | Pencil nodes |
| --- | --- |
| Library and empty library | `GaNsb`, `sI5K0`; row/header `DSUqE`, `tgdj2` |
| New pipeline page and slug collision | `lAQis`, `Hgequ` |
| Selected step, pipeline summary, empty graph | `SVBQi`, `c24Tv`, `Z26ZFs` |
| Agent/tool assignment and unavailable executor states | `Clxs2` |
| Step cards, badges and parallel groups | `r6PIM`, `Co8O6`, `qXE7Z` |
| Publish blocked, publish review, published version | `c7g6Qi`, `ITC0R`, `nmY08` |
| Version history and read-only versions | `OchSR`, `qCku8` |
| Settings page and edited description | `x4IIjM`, `Yx32O`, supported portions of `fmnva` |

Routes: `/project/<id>/pipelines`, `/project/<id>/pipelines/new`,
`/project/<id>/pipelines/<pipeline-id>`. Settings and history open from the editor toolbar.
Useful element IDs: `pipeline-new`, `pipeline-create`, `pipeline-add-step`, `step-executor`,
`step-failure`, `step-gate`, `pipeline-save`, `pipeline-publish`, `pipeline-history`,
`pipeline-settings`. Headless action recipes live in `crates/desktop-app/tests/pipelines.rs`.

The person-only save/publish questions lock MCP window input. Do not bypass that lock to obtain verification
states. The verifier can use its own seeded project or request the person's action where required.
Archive/delete states (`vl6zi`, `T7jqQY`) depend on the excluded design proposals.
No independent comparison has yet passed; move the task to Done only after the later review/Verify steps accept it.
