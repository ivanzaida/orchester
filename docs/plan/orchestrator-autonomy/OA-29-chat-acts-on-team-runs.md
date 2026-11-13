# OA-29 · The chat merges, starts and continues team runs as it does local ones

- **Status:** in progress (the chat's merge of a team run: done 2026-11-13; the rest below)
- **Kind:** workspace
- **Depends on:** OA-27, OA-28
- **Branch:** —
- **Tokens:** —

## Goal

In a team project the Orchestrator has the same hands: it merges a finished run, starts the tasks it leads,
and gets a stopped or interrupted run going again.

## Today

- The chat's merge goes through `merge_proposed` (`orchestrator/change.rs`), which refuses a team run; in a turn
  it started, `auto_merge` leaves every team run to the person (`MergeLeft::TeamDelivery`).
- `led_start` allows a start only after a merge the chat made, and reads stored tasks (`find_task`), so no team task
  is ever started in a turn Orchester started.
- `continue_run` for a team run is refused (`team_project_unsupported`; raising limits `decision_required`),
  `resume_run` of an interrupted one too, and a message that would continue a run or run its task again.
- `run_proposed` refuses a team project.

## Change

- With OA-27, a team run is merged by the chat under the rule a local run is (`auto_merge`), and
  `MergeLeft::TeamDelivery` goes; a run with team inputs and no team record stays the person's.
- `led_start`, OA-13's *work can start* and OA-14's second run read team tasks as the server gives them, and start
  them through `team::run_task` (which takes the claim).
- `continue_run` (retry, return, accept, resolve conflicts), `resume_run` and a continuing message are carried out
  for a team run under its claim. Raising limits waits for OA-34.
- Every such action is the claim holder's, recorded as automation's, as for a local run.

## Edge cases

- The claim is another member's: refused as the tool refuses a person (`not_holder`), and the task is parked
  (OA-15) with who holds it.
- The server cannot be asked: the run goes on, as the outage states of `docs/design/team-authority.md` say; the
  chat's call is refused with that reason and tried again by its event (OA-01).

## Done when

- Workspace tests with the stand-in server for each call, in a person's turn and in a turn Orchester started.
- Their rows leave the parity table. `tools/check.ps1` passes.

## Done so far (2026-11-13)

The chat's rule for merging a finished run on its own (`orchestrator/auto_merge.rs`) reads a team run as it reads a
local one, and the merge the chat or a merge card carries out (`merge_proposed`) takes a team run into the branch it
started from, its module's, without asking for the module's delivery. The run of a team task in no module lands on
the base branch itself and is merged only as `team::may_merge_on_base` allows (an Admin, or the team's Automatic
merge). `MergeLeft::TeamDelivery` stays for entries kept before.

**Left:** starts of team tasks in a turn Orchester started (`led_start` reads stored tasks), `continue_run`,
`resume_run` and a continuing message for a team run, `run_proposed`.
