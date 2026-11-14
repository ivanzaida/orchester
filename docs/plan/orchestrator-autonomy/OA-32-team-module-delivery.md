# OA-32 · The Orchestrator asks for a team module's delivery, as it delivers a local one

- **Status:** done as a call of its own; the gate before it is local only (2026-11-14)
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

## As built (2026-11-14)

`deliver_module` is one tool for both kinds. For a team module it requests the delivery from the team server as
`merge_run` of one of its runs did, without a run (`team::deliver_module`): under the module's claim, decided by an
Admin's authorization or the team's automatic merge; the tool never authorizes. In a turn Orchester started the
chat may ask for it for a team module it leads to the end once every task of the module is done.

**Left:** the acceptance before a team module's delivery (OA-20 keeps a module's acceptance on this computer and
makes its task here, which a team project's tasks are not), and parking the module for its Admin by name when the
server keeps the request waiting: the refusal is told to the chat, which tells the person.
