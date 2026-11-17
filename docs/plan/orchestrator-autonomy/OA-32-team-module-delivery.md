# OA-32 · The Orchestrator asks for a team module's delivery, as it delivers a local one

- **Status:** done (2026-11-17): the acceptance before a team module's delivery as a local module's; parking the
  module for its Admin by name is left
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

**Left:** parking the module for its Admin by name when the server keeps the request waiting: the refusal is told to
the chat, which tells the person.

## The acceptance of a team module (2026-11-17)

- A team module's acceptance pipeline (one of the team's) and *Deliver when accepted* are this computer's choice
  (`AcceptanceChoice`, the table `team_module_acceptance`): the server keeps neither. They are chosen on the module's
  Summary (*Delivery*, the module form's rows without *Its own branch*) or by `update_module`, which sends the server
  nothing for them, so a Member chooses them as an Admin does; `get_module` tells them under `own_branch`.
- `start_acceptance` reads the module's tasks from the team server, refuses as for a local module (`tasks_open`,
  `acceptance_under_way`, `already_accepted`, `no_acceptance_pipeline`), makes the task through `create_task`'s
  `task.create` and move into the module, keeps it in `module_acceptances` by its server id and starts it through
  `run_task`'s team start. Its brief is the local one: the module's tasks from the server, the steps automation
  accepted unverified from this computer's runs, the module's branch and the checked-out branch it is delivered into.
- Where the module stands is the local rule (`DeliveryFacts::step`, `crate::modules::team_delivery`): its
  acceptance's status from the server's last reading of the watched module, its runs and merges from this computer;
  then the delivery the server decides. A module without an acceptance pipeline is the person's to deliver, as a local
  one; the chat's *deliver* step outside a turn the person started needs *accept* passed first, as for a local one.
- **Needs the server's contract:** `task.create` and a task's definition route are the Admins' on the team server, so
  on a Member's computer the acceptance task (and the tasks of its findings) is refused `not_admin` and nothing is
  kept. A Member who holds the module's claim may make no task in it today. Row of the parity table.

Tests: `orchester-workspace --test team_mcp team_mcp_acceptance` and `team_mcp_a_led_module`, `orchester-app --test
team_qa team_qa_acceptance`.
