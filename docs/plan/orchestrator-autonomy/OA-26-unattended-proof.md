# OA-26 · An unattended module, end to end, with the fake agents

- **Status:** done in part (2026-11-14): a local module of three tasks, end to end; what is left is below
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

## In both kinds

The scenario runs twice: in a local project, and in a team project against the stand-in server of `orchester-api`
(`test-support`), with the same assertions, plus for the team run: the claim held through the waits (OA-31), the
tasks Done on the server after their merge (OA-30), and the module's delivery asked for or parked for its Admin
(OA-32). What the stand-in cannot show is listed as not run.

## Not covered, and said so

A real model's judgement in these turns; the window closed (OA-08); a module with its own branch (OA-17). A run with
a real model against a QA copy is a separate step the person asks for.

## Done when

- The scenario runs in the app's test binary under the heavy-suite rule (one at a time) and passes three times in a
  row.
- `tools/check.ps1` passes.

## As built (2026-11-14)

`crates/desktop-app/tests/mcp_domain/orchestrator_unattended.rs`, on the live harness of `mcp_domain` (the real
runner and the fake Codex, a project set up through the tools): a module of three tasks and a conversation that
leads it to the end. The test plays the Orchestrator's part only: in turns nobody asked for it reads
`get_standing` and calls the chat's own tools, as a leading turn's instructions say to. No person's press and no
person's answer is given.

| Task | Its agent | Where it ends |
| --- | --- | --- |
| Greet | works | started, merged into `main`, *done* |
| Sign | reports failure every time | run again twice with a reason, the third start refused `ceiling_reached`, parked (*ceiling*) |
| Publish | asks to push | parked (*approval*), its run still waiting |

It asserts: the module ends *stuck*; `main` holds the merged work; the failing task ran exactly as often as its
ceiling allows; both parkings say what is needed; every further run and every parking is a decision with its
reason; with the drive switch off the chat starts nothing by itself. It passed three times in a row (about 20
seconds each).

**Found by it:** a run that waits for a tool request reads as `working` in `get_standing`, not as waiting for a
person, so a module with such a task reads *working* until its task is parked. The turn of the request's own event
parks it; the scenario reads the requests (`list_approvals`) as that turn does. Whether the standing should say
`waits_for_person` there is a rule of OA-12 to decide; not changed here.

**Left of the scenario as planned:** six tasks with dependencies between them (2 and 3 after 1, 6 after 5) and an
agent's question answered from the brief; two agent slots with refused starts that come back; a turn of the
Orchestrator that fails and is tried again, the daily budget used and reset, a stall (each has its own test:
`orchestrator_led_waits`, `orchestrator_held`, `orchestrator_stalls`, but not in one run); the follower's own
turns with a scripted Orchestrator agent (here no Orchestrator is set up, so its events stay kept and the test
calls the tools itself); and the same in a team project against the stand-in server.
