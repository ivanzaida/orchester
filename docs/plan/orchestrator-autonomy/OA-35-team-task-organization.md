# OA-35 · Dependencies between team tasks, and the rest of a task's organization

- **Status:** built (2026-11-17): contract, server and desktop; the run-end follower and story points' source are left
  (*Left*)
- **Kind:** contract + workspace
- **Depends on:** —
- **Branch:** `claude/team-contract` (platform: `claude/claims-and-fields`)
- **Tokens:** —

## Goal

A team module's tasks can wait for one another, so the Orchestrator can order them and start them as they
become ready, as it does in a local project.

## Today

`create_task` and `update_task` refuse `blocked_by`, `tags`, `story_points` and `no_automatic_merge` for a team
task (`app_mcp/tools/team/writes.rs`): the server does not keep them. Without `blocked_by` there is no order between
a team module's tasks: OA-12's standing reads every task as ready, and OA-18's "order tasks that change the same
files" has nothing to write.

## Change

- **The contract, changed here first:** a team task keeps what blocks it (with the gate), and *Don't merge
  automatically*; tags and story points as the server models them, or said to stay out.
- The desktop reads and writes them through the tools it has; a member's change follows the server's rule for task
  details (an Admin's, or the holder's: to decide with the person).
- Until the server has it, the parity table says so, and OA-19 warns that a team module's tasks have no order.

## Edge cases

- An older server: refused as today.

## Done when

- Contract tests; workspace tests with the stand-in server. Its row leaves the parity table. The platform's change
  is listed as not done here.

## Built (2026-11-17)

Decided by the implementing agent on 2026-11-17, to confirm with the person: the server keeps what a local task keeps of
its organization, and a member's change of it is refused as for every other detail of a team task (an Admin's, the
task route's rule; the holder's own change would be a new rule, so it was not made).

- **Found first:** the server kept a team task's tags and blocking tasks already (its `task_links`, `task_tags`); the
  desktop's tools refused them only because they were not wired. Story points and *Don't merge automatically* were not
  in the contract.
- **The contract** (`orchester-api`, `docs/cloud-api.md`, "Team task organization (OA-35)"): `TaskContent` and
  `TaskRead` carry `story_points` and `no_automatic_merge`, `Organization` carries `no_automatic_merge`, and
  `TaskChanges` changes them; each is left out when none or off. `LifecycleRequest` counts them as an Admin's.
- **The server** (platform): schema 39 (`tasks.story_points`, `tasks.no_automatic_merge`), the task route, `task.create`
  and every task read.
- **The desktop:** `create_task` and `update_task` take and return them (`app_mcp/tools/team/organized.rs` resolves a
  blocking task by its key among the team's tasks and a tag by its name among the team's tags, making a tag the team
  lacks); `TaskAction::Points` sends a team task's points. `run_task` refuses a task whose blocking tasks the server
  does not hold Done, as a local blocked task is refused; `merge_run` in a turn nobody asked for is left to the person
  for *Don't merge automatically*, a Blocked mark or unfinished blocking tasks read from the server
  (`team::merge_left`); where a led module stands lists a team task blocked by a task of the same reading as
  `blocked_by`.
- Tests: `crates/workspace/tests/team_mcp/organization.rs`, `orchestrator.rs` (the merge left to the person),
  `led_module.rs` (the standing); the server's `tests/sharing/task_points.rs`.

## Left

- The follower of a kept run end (`merging`) reads this computer's records, so it still says `merges` for a team task
  the server holds back; the turn's `merge_run` is refused with the reason.
- A team task's story points keep no say of who set them, and Orchester does not estimate them.
- An older server (0.11.4 or older) refuses a patch that names the new fields, and keeps neither.
