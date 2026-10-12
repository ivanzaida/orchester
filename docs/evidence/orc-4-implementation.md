# ORC-4 Instructions implementation

This report covers the current run worktree on top of checkpoint e9614f6cc0963a2877b89925c8dc98f556e24e82 (tree 9ec74c23c6bf1a35b122e2b73a528aa3441ebe90), with the scoped registry adoption and editor changes described below. The final committed HEAD/tree/blob manifest is recorded separately in .tmp/instructions/final-manifest.json. Original task base: 934b85b92f05296b7e30552073eea438136010f0.

The Instructions route is built and listed in tests/screens.rs. It lists project, role, pipeline and step instructions with state/version; edits agent targets and timing; saves drafts with optimistic outside-change checks; publishes saved drafts; keeps invalid or conflicted typed text; supports explicit reload and in-place leave/keep; reads repository findings and creates selected drafts only; exposes real variables and composed prompt sources; and opens the full read-only prompt as an in-place page.

## Published dependency and API

Only modern workspace lurq was changed from 0.43.0 to registry 0.44.0 via `cargo update -p lurq@0.43.0 --precise 0.44.0`. Cargo.lock changes only that package/version/checksum and its two modern references. Unrelated versions/checksums, lurq_legacy 0.42.0 and deprecated desktop source are unchanged. No path/git override or vendored source was added.

Cargo's sandboxed download failed with SEC_E_NO_CREDENTIALS; the requested elevated `cargo fetch --locked` was approved and succeeded. The downloaded archive SHA-256 is 374c9b8ad378bb8a4601cd9e4ec34142db9d944008b069d9f96439639f0adac9, equal to the lock and approved uploaded archive. Registry .cargo_vcs_info.json reports afbaf46f097c69f49e50b5ec5bf5e249b0bfb67e. The locked macro remains 0.1.2. Read the actual accepted publication's docs/src/content/docs/text-input-tokens.md at H:/projects/lurq/.tmp/worktrees/codex-orc4-input-tokens; the source publication ledger records its SHA-256 935f63eef7b3d9d75185dd9788bc09d057093aa1fb465866b02fd910e1f5a266. Historical publication reports and SDK native/source passes are not app verification.

ui/prompt_input.rs retains one TextInputController in each keyed editor's Fields. TextInput.controller shares that retained instance; the app never recreates it per render or manually resets its caret. Variable insertion calls replace_selection and handles typed InputEditError and false/prevented results through localized field feedback. Accepted insertion restores ordinary runtime focus in the next render and uses the normal on_input/undo path.

The existing core prompt grammar now exposes exact byte ranges through prompt_references. Strict construction uses that same scanner and rejects the first error. Workspace inline_references chooses known inline references for decorations, leaving invalid/partial/escaped text editable and excluding ranges containing CR/LF without changing their validity or raw bytes. The UI performs no duplicate prompt validation/composition. InputTokens validates the exact current source each render; normal versus unavailable styling follows actual PreviewCatalog values. TextInput handles raw UTF-8 selection, CRLF, shaping and soft wrapping. Save/history/export/clipboard continue to use raw prompt text, not label objects.

Pencil oA91D and k3WFT map to instruction token padding/radius 3, prompt mono 12 with a 20 line pitch, pressed normal fill/text-primary and warning-muted unavailable fill/warning text. The published InputTokenStyle has no stroke property; the design's 1px pill outline is not drawn. The independent verifier must classify this visible difference; no native fidelity verdict is asserted here. No exhaustive QuadContent adaptation was needed in this app; headless tests observe the genuine SDK DecoratedInput path, with no fake variant handler.

## Validation

Command outcomes are recorded in .tmp/instructions/adoption-command-results.json. Current focused commands:
- cargo test -p orchester-core --test prompt prompt_ --locked: PASS 10.
- cargo test -p orchester-workspace --test instructions_tokens instructions_tokens_ --locked: PASS 2.
- cargo test -p orchester-workspace --test instructions instructions_ --locked: PASS 4.
- cargo test -p orchester-app --test instructions instructions_ --locked: PASS 16, including real picker selection replacement, focus restoration, undo/redo and exact draft bytes.
- cargo test -p orchester-app --test ui_kit prompt_input_ --locked: PASS 4, including Unicode byte selection, one undo step, prevented/invalid/masked/unmounted errors, stale decoration removal, CRLF and external reset.
- cargo test -p orchester-app --test ui_kit empty --locked: PASS 2.
- cargo test -p orchester-app --test screens screens_ --locked: PASS 2.
- cargo test -p orchester-app --test i18n every_key --locked: PASS 2.
- cargo test -p orchester-app --test i18n locales --locked: PASS 3.
- cargo test -p orchester-app --test i18n area --locked: PASS 4 (overlaps earlier i18n filters).
- cargo check --workspace --all-targets --all-features --locked: PASS after the app-local DevTools view-state wrapper fix.
- cargo clippy -p orchester-app -p orchester-workspace --all-targets --locked -- -D warnings: PASS after removing an unused prop and splitting the panel helper.
- cargo fmt -p orchester-core -p orchester-workspace -p orchester-app: exit 0, stable rustfmt warns about repository nightly-only options.
- git diff --check: PASS.
- Apple cross-check with CC_aarch64_apple_darwin=Clang and AR_aarch64_apple_darwin=llvm-ar: exit 1 before app code, missing Apple SDK stdlib.h/stdio.h/string.h in existing native dependencies. App macOS compilation/execution remains unverified.

Earlier development failures (a test fixture moved event.new_value before prevent_default; all-features lacked an app-local inspect marker; clippy found an unused prop/long panel render) were corrected. They are context, not verdicts on the final candidate. Preliminary cargo check -p orchester-app --locked passed before final wiring. Exact final source checks/tests are collected before handoff.

Not run: tools/check.ps1 (the pipeline Check step is authoritative), full/unfiltered cargo tests, native app/QA screenshots or design verification, macOS execution, 960 and light passes. Native Windows/Mac IME/clipboard/BiDi parity, human-only states and unreached states are Unverified. No installed app, original database, root master dirty docs/design, or new Task Details design was touched. No QA window was launched; root must lease the single disposable QA slot before Verify.

## Independent Verify inventory

Read-only Pencil data/orchester2.pen, Instructions layer fePhH:
- XMlEH: project rule editor; b1i8f library, p6Bsv heading, p7ztk tabs, ilieH/f4Slyn/qSn5T editor, cCFTK/kov6Z prompt and Un13s/blqcd toolbar/gutter, r8Ipz variable panel and jNHQ3 task.title detail. zmP3o shared rule item. oA91D/CX02m normal inline variable.
- J7EgbM: unavailable run.revision, k3WFT/OV84i warning token, explicit unavailable context, Resume choice and unavailable fallback. Validate raw text and insertion errors; no silent empty value.
- ShcL9: real task/step/run/event context, draft substitution, included/excluded sources, missing-variable and copy states.
- fTuZZ: in-place full viewer XXiGS, header CKzcD (52), source index IAt1E (300), prompt QI125, breadcrumbs/back/read-only/copy; no modal.
- udfuB: Agent roles and selected role editor.
- w20Jf: Step overrides, selected pipeline/step and default preview panel; also exercise pipeline scope tab.
- B5A4Ux: empty sG2xi/yxIJ7 and real findings T1w9U, root files plus two grouped skill folders.
- U69ob: repository review, valid/invalid selection, raw preview, busy/failure and created drafts.
- GxmNs: published v3; draft saved with v3; unsaved; invalid/not-saved; new draft; publication failure; published v4.
- Additionally: optimistic outside-conflict text retention and explicit reload; stale draft; loading/error/retry; leave/keep guard; long library/import scrolling; selecting text then variable insertion; Unicode/CRLF/soft wrap; focus/undo/redo; clipboard raw bytes; selected variable detail and normal/unavailable pill geometry/stroke difference.
- Excluded: AknZj 960, nemEA light, macOS execution.

Implementation test success and SDK publication/native success do not constitute independent source review or app design verification. Those later steps and authoritative Check must pass before merge; root owns verified merge and post-merge checks.
