# OA-09 · Core: a subscription leads to the end, with ceilings

- **Status:** done but for choosing it (2026-11-13; OA-10 and its frames, `frames.md` B; independent review: one blocking finding fixed, see *Review*)
- **Kind:** core + store + workspace
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

The person can tell a subscription to carry its work to the end, and what the chat does on its own there is bounded
by rules its tools enforce, not by its prompt.

## Today

A `Subscription` is a conversation, a target, the person's order as text and since when
(`crates/core/src/orchestrator/watch/mod.rs`). The order is quoted to the agent as data and only narrows. Since
decision 14 nothing counts the chat's own returns, retries or raises: `continue_run` takes any of them while it
drives (`crates/workspace/src/app_mcp/tools/continue_run.rs`, `tasks/raise.rs`).

## Change

- **Mode.** A subscription is *Follow* (today's behaviour, the default, and what every stored subscription reads as)
  or *Lead to the end*. Only the person sets it, with the watch command (OA-10); the agent's `watch` tool always
  makes a *Follow* subscription and cannot change a mode.
- **Ceilings** of a leading subscription, checked by a constructor, stored with it:
  - runs of one task, the first included: 1–5, 3 by default (used by OA-14);
  - returns and retries of the chat's own per run (`continue_run` `return_to`, `allow_retry`, `resolve_conflicts`):
    0–10, 4 by default;
  - raises of a limit or a step's budget per run: 0–3, 1 by default, each by at most 50 % of the limit;
  - an optional budget in tokens for the subscription's work: what the runs of its tasks and its conversation's turns
    spent since it started.
- **Counting** is read from the project's database: the runs of the task, and the chat's own actions on the run as
  the conversations keep them. It counts per run and per task, whichever conversation acted.
- **Enforcement** in one place the chat's calls pass in a turn Orchester started: a call past its ceiling is refused
  `ceiling_reached`, naming the ceiling and what was used, and the turn parks the task (OA-15). In a turn a person's
  message started the ceilings do not hold: the person asked.
- A used budget of the subscription lets what runs finish and starts nothing new; OA-12's standing says so.
- Project settings › Orchestrator › *Watching* holds the defaults a new leading subscription takes.

## Edge cases

- A subscription that moves to another conversation (*Take it here*) keeps its mode, ceilings and what was used.
- A task subscription inside a leading module subscription: the more specific one decides, as for routing.
- A *Follow* subscription keeps today's unbounded behaviour; this task changes nothing for it.

## Done when

- Core tests for the constructor's bounds; workspace tests for each ceiling's refusal and for the person's turn
  (`--test orchestrator_ceilings`).
- Store migration with a test that an existing subscription reads as *Follow*.
- `tools/check.ps1` passes. Nothing visible yet; OA-10 draws it.

## As built (2026-11-13)

- **Core** (`orchestrator/watch/lead.rs`): `WatchMode` (`Follow`, `Lead { ceilings }`) on `Subscription`, `Ceilings`
  with their bounds and the person's defaults (3 runs, 4 returns, 1 raise by 50 %, no budget), what each allows, and
  the counts read from a run's events: automation's returns and retries, and every raise (the events do not say who
  raised, so a person's raise counts too).
- **Store:** `orchestrator_subscriptions.lead` holds the ceilings as JSON, NULL for one that follows; every stored
  subscription reads as following. A take-over or a renewal keeps how the target was led.
- **Workspace** (`orchestrator/ceilings.rs`), in a turn Orchester started: `continue_run` is refused
  `ceiling_reached` past the returns or the raises, or for a raise beyond the share; a `send_message` that would
  continue a run or follow it up counts as a return, and one that would run the task again is held by the runs of
  the task; `run_task` and all of the above are refused once the subscription's work (`spent`: every turn of its
  conversation and the runs of its tasks since it leads) used its budget.
- **Not here:** `run_task` itself still refuses every second run (`already_ran`), which OA-14 replaces with the runs
  ceiling. Choosing the mode and the ceilings, and the project's defaults, are OA-10's and wait for their frames:
  until then only a test makes a leading subscription. A budget counts from when the subscription leads, so a
  take-over starts it again.
- **Known limits:** accepting a step (`continue_run` `accept`) and resuming are not counted; automation's returns
  count whoever of automation made them; the turn under way is not in what was spent; a task followed by a
  subscription of its own inside a module led to the end has no ceilings.
- **Team projects:** the ceilings read the task's leading subscription, found for a team task by the module the
  team server puts it in (OA-28), and they bound every call they bound for a local run: `continue_run` (OA-29,
  2026-11-14), `run_task`, and since 2026-11-17 a `send_message` that runs a team task again
  (`orchester-workspace --test team_mcp run_again::`). Raising a team run's limits is refused for its own reason
  (OA-34).

## Review (2026-11-13)

An independent reviewer read the change. Fixed: a `send_message` that moves a run (continues it, follows it up, runs
its task again) passed no ceiling, so the chat could return a run or run a task again without end by another door;
the budget did not stop continuations; the runs ceiling was held nowhere; a take-over left the set in memory
following while the store kept leading (one rule now, `Subscription::succeeding`). Added to the parity table: a team
task led through its module has no ceilings until OA-28.

Tests run, on the branch after `origin/master` was merged: `orchester-core --test orchestrator_ceilings ceilings`;
`orchester-store --test orchestrator_subscriptions subscriptions`; `orchester-workspace --test orchestrator_ceilings
ceilings`, `--test orchestrator_led_waits led_waits`, `--test orchestrator_watch watch`, `--test
orchestrator_auto_merge auto_merge`; `orchester-app --test mcp_domain` for `orchestrator_ceilings::`,
`orchestrator_guard::`, `orchestrator_continue::`, `orchestrator_chat::`, `orchestrator_sent::` (30 passed);
`tools/check.ps1`: OK. Not run: a raise past its share or its count against a run stopped at a limit (the seed has
none; the rule is tested in core), the budget refusing a start through `run_task`, a message that runs a task again,
macOS (no `cfg` code).
