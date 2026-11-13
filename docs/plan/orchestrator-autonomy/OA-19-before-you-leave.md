# OA-19 · Before you leave: is what the conversation leads ready

- **Status:** todo
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
