# ORC-3 Implement13: five presentation corrections

Base: 703367ef55afd0ee393944a4e4e3ddbde965076a (product 43e51b4c).
This handoff describes the product and test changes in the commit containing this file.
The final Implement response supplies that exact commit. The run remains pending independent
Check, Review and Verify; this is not a native design-verification verdict.

Read both full reports, keeping their overlapping identifiers distinct:

- .tmp/verify/orc3-independent-703367ef/report.md: independent V13-M1/M2.
- .tmp/verify/orc3-verify13-703367ef/report.md: pipeline V13-M1/M2/M3.

## Changes

- Independent M1: publication-problem buttons now cap their width and ellipsize a long label.
  Full accessible names are retained; clicking opens the matching inspector and its full editable
  name. Every short and long fixture target remains selectable.
- Independent M2: graph titles and executor metadata stay on separate bounded lines, with ellipses
  for long values. The existing card/connector geometry therefore remains valid for legal
  200-character names. Full names remain accessible in the selected step inspector. Domain Name
  limits are unchanged.
- Pipeline M1: Commands uses the compact selector and summary, with a checkbox popover opened
  from it; the section identifies the profile and narrowed state, and its note names the profile's
  supported command list. Unsupported network control stays inert, with its capability reason in
  a tooltip. Reset/recovery and workspace validation, save and publication guards remain intact.
- Pipeline M2: empty library offers Create from starter and Start empty. Both navigate to the
  existing creation page, preselect the corresponding choice, and create only on Create draft v1.
  Empty selection is preserved in the route query. The starter uses dependency-level groups,
  including a stacked parallel checks/review group, and restores the draft/publication explanation.
  It truthfully retains the domain starter's six steps and two gates rather than the illustrative
  design's different count.
- Pipeline M3: no draft row appears without saved or local unpublished work; viewing the latest
  version without a draft cannot compare it to itself. It shows disabled Compare with draft and
  the next-edit version reason. First-version metadata is truthful. Restore remains supported in
  a wrapping bounded action row with unchanged replacement/person-only safeguards. Comparing an
  older published version to the current published version still works and retains H3 provenance.

## Checks on these product/test inputs

All commands below were run freshly in this worktree, not reused from a prior candidate.

| Command | Result |
| --- | --- |
| cargo check -p orchester-app | PASS |
| cargo fmt --all | Failed before formatting: Windows command-line length, OS error 206 |
| cargo fmt -p orchester-app | PASS, with stable-toolchain warnings about nightly-only formatting options |
| cargo +nightly fmt -p orchester-app | PASS (run after edits; final formatting used by Check) |
| cargo test -p orchester-app --test pipelines pipelines_ | Initial 40 passed / 1 new-test failure: autosave wait indexed an initially empty saved draft. Fixed the wait; final rerun 41 passed |
| cargo test -p orchester-app --test ui_kit fields:: | PASS, 11 |
| cargo test -p orchester-app --test ui_kit blocks:: | PASS, 13 |
| cargo test -p orchester-app --test routes every_route | PASS, 2 |
| cargo test -p orchester-app --test i18n every_key_named_in_code_has_its_area_file_and_exists | PASS, 1 |
| cargo test -p orchester-app --test i18n locales_ | PASS, 3 |
| cargo test -p orchester-workspace --test pipeline_writes pipeline_writes_ | PASS, 3 |
| powershell -File tools/check.ps1 | PASS, exit 0, check: OK; formatting, workspace/all-target/all-feature clippy, size and UI literals |
| git diff --check | PASS |

The final pipeline tests and Check cover the final Rust/locale inputs. Other filtered tests ran on
unchanged product inputs before the final test-only wait/reproducer refinements.

New regressions exercise the exact Long step + 190 W case (and a second legal 200-character name),
actual mouse clicks at problem-action centers, label/card bounds, vertical title/metadata separation,
both empty-library entry selections and resulting draft bodies, first-version/no-draft and subsequent
real-draft comparison, and compact command selection with real enforcement.

Not run: full unfiltered suites, native QA or screenshots, macOS cross-build/runtime, host-focus
inspection, protected native decisions, 960 or light. Headless guard tests are not native acceptance.

## Verify next

Read-only Pencil Get/screenshots were used from the live H:/projects/orchester/data/orchester2.pen.
No design mutation or direct encrypted-file read occurred.

At Windows dark 1440 x 900, independently compare:

- c7g6Qi: all three short publication problems and eight-problem wrapping; repeat Long step plus
  190 W, select every link, inspect full name, check ellipsis and separation of graph title,
  profile/access metadata, following node and edges. Compare normal graph structure to SVBQi.
- K0WLeG within SVBQi: inherited and narrowed commands; closed compact summary and opened choices;
  profile context, capability reason, network unavailability, reset recovery. Do not bypass a guard
  to obtain a capture.
- sI5K0: both empty-library actions, grouped parallel starter and publication explanation.
  lAQis: each entry opens the same New pipeline page with the correct selected option;
  Hgequ collision and Z26ZFs empty created draft remain regression states.
- z0iXz within qCku8: published v1 alone with no draft, truthful first-version metadata, disabled
  comparison/reason and bounded Restore. OchSR: saved draft, historical comparison and H3 provenance.
- Preserve the existing Clxs2/iUiLa/adK4e/A0a2p executor readback/unavailable states and
  c24Tv/Z26ZFs Summary fixes; normal executors in SVBQi.
- Pipeline settings layer vEDYY (x4IIjM/Yx32O) is unchanged; retain existing exclusions and
  previously unverified states rather than relabeling old screenshots.

Open profile remains explicitly unavailable with its explanatory tooltip pending ORC-2.
Do not add a profile page. F1–F4, H3 and person-only boundaries remain required.

No QA processes were started or controlled in this implementation step. Preserve foreign pending
PIDs 35224/42916, refused approvals 10/13/14 and cleanup denial 25. Do not retry their classifier or
protected actions. Earlier reports' QA identities and native Unverified states remain historical
and unchanged, including protected acceptance, host focus, errors/races and omitted platform passes.
No installed app, production DB, global input, whole-screen capture, master merge or replacement run
was touched. Preserve newer master's focus/compact/masked Fields and Tools/Secrets at eventual
integration. Do not move ORC-3 to Done until independent verification clears the candidate.
