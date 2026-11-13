# OA-35 · Dependencies between team tasks, and the rest of a task's organization

- **Status:** todo
- **Kind:** contract + workspace
- **Depends on:** —
- **Branch:** —
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
