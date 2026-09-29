# ORC-44: a user decision reported as failure

## Observed run

Run `01a0eeb3-db2f-7397-83e6-ad96fe4dab2e` completed its Implement agent invocation, but the agent's
structured result said `failed`. Its unresolved issue was whether command approval may authorize execution outside
the sandbox. The user subsequently clarified that an explicitly approved command may do so; this is distinct from
turning off the sandbox for all agents or commands.

The runner recorded `reported_failed`, saved checkpoint `9b3484f1f85cdcefcdcd237984a9c0bcbbd07453` on
`orchester/orc-44-run-1`, and skipped the later Check, Review and Verify steps. The implementation work remains in
that checkpoint. Its earlier tests used a fake CLI and do not constitute independent live verification of ORC-44.

## Cause and correction

There is no separate semantic classifier of the agent's summary. `StepResult` parses the structured outcome, and
`execution/step/end.rs` routes a successful invocation's `question` to a checkpoint and a waiting conversation;
`failed` invokes the pipeline's failure handling. A failed CLI invocation remains an execution failure.

The original prompt defined `failed` broadly as inability to complete the step. Resumed turns got either a vague
reference to the previous result instructions or no result instructions at all. Resumed Codex CLI turns also do not
receive an output schema.

The correction on `codex/attention-outcomes`:

- Defines `question` for a needed user decision, permission, access, clarification, or resolvable scope conflict.
  It asks for a concrete question and options, with the blocker and saved progress.
- Defines `failed` as failure with no user decision or action that would enable continuation, and describes its
  possible terminal consequences. `done` means the current step is complete.
- Requires the same explicit JSON outcome contract on new turns and every resume path, including plan revisions,
  checkpoint resumes without messages, answers and follow-ups. Gated steps retain the complete-plan requirement.
- Adds descriptions to the existing JSON schema without changing its fields or enum values.

Requesting permission does not grant it. This patch changes neither effective permissions nor approval transport;
it does not execute commands outside the sandbox. The user's per-action permission belongs in ORC-44's interactive
approval implementation.

## Verification and limits

An independent agent reviewed the candidate and found no blocking or major issues. It ran these Windows checks
using the candidate worktree's existing target directory:

- `cargo test -p orchester-core --test run prompt --locked`: 13 passed, including the three new outcome tests.
- `cargo test -p orchester-core --test run conversation --locked`: 14 passed.
- `cargo test -p orchester-core --test run plans --locked`: 10 passed.
- Runner tests below each passed, using `cargo test -p orchester-runner --features test-agent` with the named
  `--test` binary and filter:
  - `conversation conversation_a_codex_question_waits_and_the_answer_resumes_the_session`.
  - `conversation conversation_a_claude_question_waits_and_the_answer_resumes_the_session`.
  - `graph graph_a_reported_failure_retries_and_then_passes`.
  - `tools tools_command_step_without_retries_left_fails_the_run`.
- After correcting the schema's wording to account for pipeline retry routes,
  `cargo test -p orchester-core --test run outcome_prompt --locked`: 3 passed again.
- Final `powershell -File tools/check.ps1`: `check: OK`; `git diff --check`: clean.

These tests check the outgoing contract and existing runner question/answer and failure routes. They use protocol
fixtures; no live model compliance test was run, so they do not prove that a model always chooses the right outcome.
No Mac test was run; the changed code is platform-independent prompt composition. This change does not reinterpret
prose, rewrite previous events, or reopen ORC-44's failed run. There are no UI changes or Pencil edits. The installed
stable build is unchanged.
