# OA-19 · Before you leave: is what the conversation leads ready

- **Status:** built (2026-11-14); design verification pending (a separate agent verifies)
- **Kind:** design + workspace + UI
- **Depends on:** OA-09; the frames of `frames.md` C for its visible part
- **Branch:** —
- **Tokens:** —

## Goal

The person learns what will stop the work while they are away before they go, not the next morning.

## Today

Nothing checks a subscription. A missing switch, a task without a pipeline, a signed-out connection or a closed
Pencil is found by the first run that needs it.

## Change

- **Workspace.** One reading of a leading subscription that lists what would stop or weaken it, each with where to
  fix it:
  - *Let the Orchestrator drive* is off; a second switch its turns need is off (until OA-11);
  - the Orchestrator's connection is not signed in, or has no second one to go on with (after OA-04);
  - a led task has no pipeline, or its pipeline is not ready;
  - a led task's pipeline has no Deliver step: its merge is not checked on the merged result;
  - the target's tasks merge into the person's checkout, which has local changes now (no branch of its own, OA-17);
  - a profile of a led task's pipeline chooses an MCP server that is off or whose program is not found (the design
    tool a Verify step reads);
  - no daily budget is set, or what is left of it is less than the target's open tasks spent on average;
  - the app stops when its window closes, and the computer may sleep (until OA-08).
- **Seen** where the person makes the subscription a leading one and in `/watching`: *Ready to leave* or the list,
  each line with its place. It warns and never refuses: the person may lead anyway. `get_watches` gives the same.
- It is read again when the list is opened; nothing polls it.

## Design first

On `Orchestrator watch · redesign` `QOPcg`: the card under the leading line with its lines and their links, the
all-clear state, and the same inside `/watching`.

## Edge cases

- Whether an MCP server answers cannot be known without starting it; the reading says only what the app knows (off,
  program not found) and names the rest as *not checked*.
- A line the person cannot fix here (a team's setting) says who can.

## Done when

- Workspace tests for each line and the all-clear (`--test orchestrator_readiness`); app tests for the card.
- An independent design verification. `tools/check.ps1` passes.

## As built (2026-11-14)

One reading of a subscription that leads to the end (`orchestrator::leaving::leaving_in`), made when it is asked
for; nothing polls it. `get_standing` gives it as `before_you_leave` for each such subscription: `ready`, the lines
(`weak`), each by its name with its words and the tasks it names, and what the app cannot check (`not_checked`).

The lines, the most stopping first: *Let the Orchestrator drive* is off; the Orchestrator has nothing it can run on;
tasks that are still to start have no published pipeline; their pipelines end in no Deliver step; the work lands in
the branch the person's checkout has and the checkout has uncommitted changes now (a module with a branch of its own
does not have this line); the project has no daily budget; the computer may sleep (OA-08's setting is off); and,
always, closing the window stops the work. That last line alone leaves the reading *ready*: it is what the person
must not do.

**Not read, and said so in `not_checked`:** whether an MCP server a task's profile chooses is on, found, and
answers. A start's own readiness tells that task by task (`Workspace::ready_to_start`), with the keychain; reading
it for every task of a module here was left out.

**Not built:** the card under the leading line and inside `/watching` (frames C); the comparison of what is left of
the daily budget with what the open tasks spend on average; a second connection (OA-04, later); a line for a team's
setting that only an Admin can change.

Tests: `orchester-workspace --test orchestrator_leaving leaving`.

## The card, as built (2026-11-14)

Frames: `Parts · Before you leave` `k0jNa` a–d, under the leading line in `L01` `QMwUB` and inside the `/watching`
answer in `L02` `HpcQJ` (between the total and the list).

- `screens::orchestrator::watch::leaving`: a component per target led to the end, which reads
  `leaving::leaving_of` (the reading of the conversation's subscription to that target, `None` when it does not lead
  it to the end) when it shows and again on *Check again*, off the UI thread, with no agent turn. Under a
  conversation's lines it shows under the latest line that led the target to the end.
- The card's lines are the app's own short words for each finding, with the place that fixes it: the switch →
  Project settings › Orchestrator; nothing to run on → Orchestrator settings; tasks without a pipeline → the first
  of them; a pipeline without a Deliver step → that pipeline, a line per pipeline (`Weak::NoDeliverStep` now names
  the pipelines beside the tasks); no daily budget → Project settings › Runs; the computer may sleep → Settings ›
  General. With nothing found: *Ready to leave — nothing found that would stop Webhooks v2 · not checked: …*.
- **Differs from the frame:** *Your checkout has local changes* has no link: the frame draws *Local integration ↗*,
  but Local integration is a page of one run whose merge waits there, and nothing in the app commits or stashes the
  checkout. *Pencil design is turned off* is not a finding the reading has (`not_checked` names the MCP servers
  instead: *Not checked: whether the tasks' MCP servers answer*). The count leaves out *Closing the window stops the
  work*, which is always listed (the frames count it in a and not in L01).

Tests: `orchester-workspace --test orchestrator_leaving leaving`; `orchester-app --test orchestrator leading`.