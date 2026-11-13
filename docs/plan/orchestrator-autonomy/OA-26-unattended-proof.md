# OA-26 · An unattended module, end to end, with the fake agents

- **Status:** todo
- **Kind:** tests
- **Depends on:** OA-01 … OA-03, OA-06, OA-07, OA-09, OA-12 … OA-15, OA-18, OA-21, OA-22
- **Branch:** —
- **Tokens:** —

## Goal

Before anyone leaves a real module to the Orchestrator, one test shows a module going from "the person subscribed"
to "every task merged or parked" with no input from a person, through everything that went wrong on the way.

## The scenario

A seeded local project (a new scenario of the QA seed, `cargo run -p orchester-app --example seed -- --list-scenarios`)
with a module of six tasks and these dependencies and behaviours of the fake agents
(`orchester-runner --features test-agent`):

| Task | What happens to it |
| --- | --- |
| 1 | succeeds; 2 and 3 wait for it |
| 2 | its agent asks a question the brief answers, then succeeds |
| 3 | fails once, is returned, succeeds |
| 4 | its agent asks to push: a person-only request |
| 5 | fails every time: past its ceiling |
| 6 | waits for 5 |

Around them: two agent slots, so starts are refused and come back; one Orchestrator turn that fails and is tried
again; the daily budget used once and reset; a stall.

The Orchestrator is a scripted agent, as `crates/runner/tests/chat_codex.rs` scripts Codex: it calls the tools a
leading turn would call for each event. The test proves the app's side, not a model's judgement.

## It asserts

- 1, 2 and 3 are merged; 4 and 5 are parked with what is needed; 6 reads *waits on a parked task*; the subscription
  is stuck, raised once.
- No event is lost: each has a completed turn, or was left to the person after its tries.
- No person's input was needed, and none was simulated.
- Every judgement is a decision with a reason; no ceiling was passed.
- A second run of the scenario with the drive switch off changes nothing by itself.

## Not covered, and said so

A real model's judgement in these turns; the window closed (OA-08); a module with its own branch (OA-17). A run with
a real model against a QA copy is a separate step the person asks for.

## Done when

- The scenario runs in the app's test binary under the heavy-suite rule (one at a time) and passes three times in a
  row.
- `tools/check.ps1` passes.
