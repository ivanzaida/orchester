# ORC-3 Implement11: publication problems

Implementation candidate: 43e51b4cb26b5577660745c67a11243abd25ea4b, based on
8bb6ebfbe5b5c3b46be4c01693888e169691acda. The saved follow-up changes this handoff
only; implementation and test inputs are unchanged. This is implementation evidence,
not an independent Review or native design-verification verdict. ORC-3 remains pending.

## Corrections

- V10-M1: the problem strip flows onto further rows, and executor links name the
  affected step instead of repeating raw UUID diagnostics. All three supplied
  problems remain selectable. An additional eight-problem fixture exercises wrapping.
- V10-M2: workspace loads draft-only profile identity/model separately from published
  assignables. The inspector shows its name and draft v1, warning outline, amber
  publication notice, inherited draft model, and provenance. Publication validation
  still refuses the draft. **Open profile remains unavailable:** this candidate has
  no profile editor route and Agents is an unbuilt placeholder. The button exposes
  that reason through a tooltip. The person authorized retaining this ORC-2 dependency
  limitation and proceeding to Check/Review/Verify. Do not add a draft-profile page or
  bypass ORC-2. This decision does not declare V10-M2 passed; independent review must
  assess the exact candidate and retained limitation before merge.
- V10-M3: unsupported tools show the unavailable executor notice, a locked graph
  badge and an unused model field. The explanation names database/HTTP/access-rule
  limitations as appropriate. Runnable command tools retain their normal presentation.
- V10-M4: shared inspector value text fills its value-column width without vertical
  flex sizing. Steps metadata uses the design's ordinary property typography.
  The regression uses publication-problems and 3 · 0 approval gates at 340px.

No safety/write, restore, history provenance/reset, protected-input or installation
logic was changed. New styles reuse theme roles and all added copy is in pipelines
i18n. No encrypted design files or deprecated app files were edited.

## Commands and outcomes

- `cargo check -p orchester-app`: initial compilation failed on a wrong ToolKind
  variant; corrected to Database. Subsequent compilation and Check passed.
- `cargo fmt --all`: failed with Windows command-line length error; used scoped
  nightly formatting instead.
- `cargo +nightly fmt -p orchester-app -p orchester-workspace`: PASS.
- `cargo test -p orchester-app --test pipelines pipelines_`: final PASS, 37 tests.
  Initial run: 34/36 passed; corrected unsupported-tool badge wording and restored
  the normal command branch's no-model presentation. Intermediate rerun passed 36.
- `cargo test -p orchester-app --test pipelines publication_states::pipelines_`:
  PASS, 4 before adding the wrapping regression; a later compile attempt found a
  missing mutable fixture DB binding. Corrected; all 5 are included in final 37 PASS.
- `cargo test -p orchester-app --test ui_kit blocks::`: PASS, 13.
- `cargo test -p orchester-app --test ui_kit fields::`: PASS, 11.
- `cargo test -p orchester-workspace --test pipeline_writes pipeline_writes_`: PASS, 3.
- `cargo test -p orchester-app --test i18n every_key_named_in_code_has_its_area_file_and_exists`:
  PASS, 1.
- `powershell -File tools/check.ps1`: final PASS. Earlier iterations failed on
  function length, a needless by-value argument and fixture-helper unwraps; all fixed.
- `git diff --check`: PASS.
- `powershell -File tools/qa-app.ps1 -Name orc3-implement11 -Scenario pipelines -NoRunner`,
  with LOCALAPPDATA scoped to .tmp/implement11/localappdata: build and launch PASS.
  Own PID 10092; project 01a12b9a-2c1c-70c2-ab5d-da3e7a65eb0a.
- `python tools/lurq-mcp-client.py 10092 lurq_windows --app orchester-app --discovery-dir .tmp/implement11/localappdata/orchester-app-mcp`:
  blocked by PermissionError reading discovery. No native screenshots or UI inputs.
- Cleanup verified PID 10092's exact executable under .tmp/qa/orc3-implement11/bin,
  stopped that owned PID, and waited for exit successfully. No foreign or installed
  process was touched. Existing pending PIDs 35224 and 42916 were not controlled.

Not run: full suites, additional MCP/domain suites, seed tests, macOS cross-check
or native run, light/960, protected acceptance, host-focus verification, native
screenshot comparison. Prior images/results are not relabeled for this candidate.
Master baed9864 was not merged; its focus/masked/compact-input and Tools/Secrets
changes must be preserved at integration.

## Independent Verify targets

Read-only Pencil Get and screenshots were used for layers i4jt6V / vEDYY and
adK4e, A0a2p, c7g6Qi, c24Tv. No write design operations were used.

At Windows dark 1440×900, use the existing pipelines fixture:

- Publication problems summary: c7g6Qi problem strip and c24Tv / Z26ZFs Summary;
  inspect full slug, zero-gate metadata and all three problem links.
- Missing profile, unpublished profile and unsupported tool: Clxs2 / iUiLa /
  adK4e / A0a2p. Check both selector identity and full warning/model readback, the
  action's decided availability, unavailable tool notice and graph badge.
- Normal runnable command and selected agent: SVBQi. Ensure their presentation and
  selection/model reset behavior survive.
- Preserve F1–F4 and H3 coverage in subsequent Review; history OchSR / qCku8 and
  Settings x4IIjM / Yx32O remain regression targets, without claiming a new native
  pass here. H2/publication/window decisions remain native Unverified unless a
  person acts. Do not retry approvals 10/13/14 or denied taskkill25.

The supported unsupported database differs from the drawing's planned pull-request
tool: compare unavailable structure while preserving the truthful database explanation.
The scope decision is resolved: keep this task within Pipelines and retain the visibly
unavailable Open profile action while ORC-2 is pending. No new profile page is authorized.

On resumption after the person's decision, git confirmed the clean 43e51b4c candidate.
Only this handoff changed. The listed filtered test passes are reused solely because
implementation and test inputs are unchanged; no tests were rerun on resumption.
A fresh `powershell -File tools/check.ps1` passed on the resumed candidate.
The documentation commit was blocked by permission to create the worktree git index
lock. This handoff remains saved, uncommitted; HEAD remains 43e51b4c.
