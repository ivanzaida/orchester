# OA-29 · The chat merges, starts and continues team runs as it does local ones

- **Status:** done but `run_proposed` and a run end's `merging` for a Member's run in no module (2026-11-17; below)
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

A team task's *Blocked* mark is the server's, and the task as this computer records it never holds it: before a
merge nobody asked for, the chat asks the server (`auto_merge::left_on_server`) and leaves a blocked task's run to the
person; a server that cannot be read refuses the merge.

**Review (2026-11-13, independent):** found that the rule read a team task's Blocked mark from records that never
hold it (fixed as above); that the kept entry says `merges` for a blocked task and for a Member's run in no module,
which only the server can tell (a row of the parity table, OA-35); and that nothing runs the merge itself for a team
run in a test (still open: the test binary has no harness for the chat's own calls on a team project). Its doubt
about the module read from the run's base source does not hold: the runner makes a team run's base source from the
module the server accepted (`crates/runner/src/execution/coordinator/accepted.rs`).

**Left:** starts of team tasks in a turn Orchester started (`led_start` reads stored tasks), `continue_run`,
`resume_run` and a continuing message for a team run, `run_proposed`.

## Done on 2026-11-14

`continue_run` (retry, return, accept, resolve conflicts, and the resume after the daily budget), `resume_run` of an
interrupted run and a message that continues one are carried out for a team run as for a local one, under the claim
its work started with: what its task's page already did (`screens/task/header/controls/action.rs`), and the tools
refused. Nothing of the old sharing stream is taken; the run's heartbeat renews its claim once it works again, and a
claim lost meanwhile fences the run. A run the team server fenced is refused (`claim_fenced`); raising its limits
stays its Admin's (`decision_required`, OA-34).

**Still left:** a message that runs an ended team task again (`team_run_again_unsupported`: the server's work start
takes no message), `run_proposed` for a team project (a start is not proposed from this computer's copy), and the
start of a team task in a turn Orchester started, which is OA-28's rest (the start rule finds a team task by the
server's reading).

## Done on 2026-11-17

- **A message that runs a team task again** (`send_message` to its run that ended): the message is this computer's
  content, so it is kept here for the task's next run, as a local run again keeps it, and that run starts through the
  team server as `run_task` starts it (`app_mcp/tools/team/again.rs`: the start is checked first, refused as
  `run_task` refuses it before anything is kept, then its claim and `work.start`). The runner gives a message kept for
  a task's next run to that run's agents whichever way the run started. A start the server or the runner takes later,
  or refuses once the message is kept, leaves the message queued for the next run, with why.
  `team_run_again_unsupported` is gone.
- **The ceilings (OA-09)** bound it as they bound a local run again: one more run of the task, refused
  `ceiling_reached` past its ceiling in a turn Orchester started.
- **The start of a team task in a turn Orchester started**, and `park_task`, `wait_on_task` and a task the chat
  creates coming back to the conversation that watches, find a team task that never ran here and that nothing
  watches by asking its team server (`orchestrator/team_task.rs`, `found`), as the chat's `run_task` and `watch` did.
- **A kept run end's `merging`** reads the server's Blocked mark from the last reading of what is watched (which
  holds every task a subscription leads), so it says the run is left to the person, not that the chat merges it.

Tests: `orchester-workspace --test team_mcp` `run_again::`, `chat_named::`, `led_reads::` (with the whole
`team_mcp_` binary, 153 tests).

**Still left:** `run_proposed` for a team project; and a run end's `merging` for the run of a task in no module that a
Member may not merge (no Admin, the team's *Automatic merge* off) still says `merges`, and the turn is then refused
`admin_required`. Closing it needs the reading to hold the member's role and the team's merge policy, and a reason of
its own in `MergeLeft`, which the run end's card shows (a UI change, with its design verification).
