# ORC-4 Review attempt 2 corrections

Saved work on parent HEAD 8d5fd5f9965203aef9afbe0a6d053870c59e7c7b, without a commit.
This report supplements, and does not replace, orc-4-review-corrections.md or its earlier failures.

## Preview recovery

The preview's unavailable state now retains the shared Task select and its context heading.
Switching tasks clears the old step/run selection as before. Step/run controls, composition,
Copy and the full viewer remain unavailable until workspace resolution succeeds. No domain
rule, scanner permission, locale, theme or deprecated-app source changed.

A mixed-validity headless regression starts with an invalid first task, selects a valid task,
then switches through a task without a pipeline, an unpublished pipeline and a published
tool-only pipeline. Each retains Task selection and recovers the preview and full viewer.

Git blob identities for this saved source:
- crates/desktop-app/src/screens/instructions/preview.rs: 0dd21d7cf2078c98baee127e82717fdbe52be20e
- crates/desktop-app/tests/instructions/preview.rs: e04065236dde811fb0a0d7983f46f20b2ae4d6bb

## Commands and outcomes in this session

| Command | Outcome |
| --- | --- |
| git status --short | PASS, exit 0; initially clean, later the two preview files modified |
| cargo test -p orchester-app --test skills skills_empty_states_ --locked | FAIL, exit 1; 0 passed / 1 failed, timed out waiting for Skills page |
| cargo test -p orchester-app --test instructions instructions_preview_keeps_task_selection_ --locked | FAIL, exit 1; first fixture incorrectly tried to publish an empty pipeline (Publish(NoSteps)); fixture corrected to a published tool-only pipeline |
| cargo test -p orchester-app --test skills skills_empty_states_ --locked | FAIL, exit 1; temporary scanner preflight reported PermissionDenied, Windows code 5, for an OS-temp repo-kontur fixture; diagnostic assertion restored exactly afterward |
| cargo test -p orchester-app --test instructions instructions_preview_keeps_task_selection_ --locked | FAIL, exit 1; missing Column-to-Element conversion in the extracted Task field; corrected |
| cargo test -p orchester-app --test instructions instructions_ --locked | PASS, exit 0; 30 passed, including mixed-validity recovery; followed by formatting-only test edits |
| git diff --stat | PASS, exit 0 |
| cargo +nightly fmt -p orchester-app -- --check | FAIL, exit 1; new regression formatting only; applied the reported formatting through edit_file |
| cargo clippy -p orchester-app --all-targets --all-features --locked -- -D warnings | PASS, exit 0 |
| cargo +nightly fmt -p orchester-app -- --check | PASS, exit 0 on the saved source above |
| git diff --check | PASS, exit 0 |
| cargo test -p orchester-app --test instructions instructions_ --locked | PASS, exit 0; repeated after formatting: 30 passed on the exact saved source identities above |
| git diff -- crates/desktop-app/src/screens/instructions/preview.rs crates/desktop-app/tests/instructions/preview.rs | PASS, exit 0; self-reviewed scope |
| git rev-parse HEAD | PASS, exit 0; parent HEAD above |
| git hash-object crates/desktop-app/src/screens/instructions/preview.rs crates/desktop-app/tests/instructions/preview.rs | PASS, exit 0; identities above |

## Remaining blocker and checks not run

Skills coverage is still failing. Its OS-temp fixture reaches a scanner access denial. No
scanner bypass, elevation, permission change or alternate-context retry was attempted.
Requested explicit approval to place the private Skills test fixtures under this worktree's
.tmp/skills/ (matching the scratch-file rule) and rerun the same assertions. That fixture
change has not been made pending the person's answer.

No check.ps1: the Implement-step override reserves it for authoritative pipeline Check.
No unfiltered Cargo tests, new workspace/scanner runs, Apple cross-check, macOS execution,
native app launch/capture, or design-verification verdict. Existing workspace/scanner coverage
in the previous evidence report applies only to its unchanged inputs. Four previously skipped
link fixtures remain Unverified. No task-level acceptance is claimed.

## Independent Verify handoff

Read-only Pencil layer fePhH in data/orchester2.pen; this session read ShcL9's expanded
context and took its Pencil screenshot. Compare at 1440 x 900:
- ShcL9 / kryoE context, jdFwY Task label and A3pabl task select: valid preview; initial invalid
  task; selected task with absent/unpublished pipeline; selected task with no usable agent step;
  returning to a valid task. Task must remain operable, errors visible, obsolete step/run controls
  absent, and composition/Copy/viewer unavailable until valid.
- fTuZZ: recovered full viewer contains the valid task's effective prompt; Back returns in place.
- The complete Instructions inventory remains in orc-4-review-corrections.md: XMlEH editor,
  J7EgbM unavailable variable, udfuB roles, w20Jf step overrides, B5A4Ux empty state,
  U69ob import and GxmNs save/publish states, alongside ShcL9 and fTuZZ.
- Native Windows/macOS parity and app/Pencil comparison remain for independent Verify.
