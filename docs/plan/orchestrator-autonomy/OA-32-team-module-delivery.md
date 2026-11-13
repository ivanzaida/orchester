# OA-32 · The Orchestrator asks for a team module's delivery, as it delivers a local one

- **Status:** todo
- **Kind:** workspace
- **Depends on:** OA-17, OA-20, OA-27
- **Branch:** —
- **Tokens:** —

## Goal

A module the Orchestrator led to the end reaches its base branch by the same rule in both kinds: accepted
first, then delivered when the person allowed it.

## Today

A team module's delivery is asked for by `merge_run` of one of its runs (`app_mcp/tools/team/deliver.rs`): the
module's claim holder, authorized by an Admin's `delivery.authorize` or by the workspace's *automatic merge* (D7); it
is refused while runs of the module work here, or while the checkout is on the module's branch. The chat cannot reach
it in any turn. A local module has no delivery at all until OA-17.

## Change

- Delivery is a call of its own for both kinds (a tool `deliver_module`, and the module's Summary), separate from
  a run's merge (OA-27). For a team module it goes through the server as today; for a local one it is OA-17's.
- The Orchestrator asks for it under OA-20's rule in both kinds: every task done, the acceptance passed, *Deliver
  when accepted* set for the module. For a team module that setting is the Admin's authorization or the workspace's
  automatic merge: the chat never authorizes anything.
- Not authorized: the module is parked for the Admin (OA-15), named.

## Edge cases

- The person's checkout has local changes the delivery touches: left to the person, as for a local module.
- Another member holds the module's claim: refused, parked.

## Done when

- Workspace tests with the stand-in server for the three outcomes (delivered, not authorized, refused by the
  checkout).
- Its row leaves the parity table. `tools/check.ps1` passes.
