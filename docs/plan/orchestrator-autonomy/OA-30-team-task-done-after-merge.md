# OA-30 · A team task is Done once its work is merged, as a local one is

- **Status:** done but the two items under *Left* (2026-11-17)
- **Kind:** workspace (and the contract, if the server must agree)
- **Depends on:** OA-27
- **Branch:** `claude/team-lifecycle`
- **Tokens:** —

## Goal

A team task whose run was merged does not sit in Backlog until someone presses *Mark as done*, and a
subscription that leads it ends by itself.

## Today

For a local task the merge sets its status. For a team task Done is the server's: its holder or an Admin sets it
(D4, built), and the server sets it by itself on a delivered module or a report-only run (`docs/cloud-api.md`). After
a merge nothing is sent: "whether a merged team run makes its task Done is open (D4)"
(`crates/workspace/src/tasks/merges.rs`). A watch on the task therefore never ends, and its module's watch neither.

## Change

- **To decide with the person (D4):** Done at the merge into the module's branch, as a local task, or at the
  module's delivery. Built as the first unless they say otherwise: the same as a local project, and the server's own
  rule at delivery then finds the task Done already.
- The holder's desktop sends the status once the merge is recorded, through the automatic statuses it already sends
  for the old sharing stream (`sharing/project/automatic.rs`); a refusal is kept and shown, never dropped.

## Edge cases

- The claim went stale between the merge and the status: the status is refused `not_holder`; the task is parked
  for the person with *Mark as done* as what to do.
- The task was reopened on the server meanwhile: the server's answer stands.

## Done when

- Workspace tests with the stand-in server: Done after the merge, the refusal kept, a watch that ends.
- Its two rows leave the parity table. `tools/check.ps1` passes.

## Found (2026-11-14), before building

The contract lets the holder of a task's claim, or an Admin, set `workflow` to `done` (`task.complete`,
`docs/cloud-api.md`), and records who closed it as `closed.how = person {by}`, the caller of the patch. A Done the
desktop sends by itself after a merge would be recorded as that person's own. So beside D4 (Done at the merge into
the module's branch, or at the module's delivery) the person has to say how the server records a Done that
automation sent: as the holder's, or as a kind of its own, which is a change of the contract. Not built until then;
the task page's *Mark as done* stays the way.

## Built (2026-11-17)

The person demanded the Orchestrator lead team projects to the end as local ones (`docs/design/local-and-team-parity.md`,
"The rule"), so D4 is taken as built first: Done at the merge into the module's branch (or the base for a task in no
module), recorded as the holder's own, with no change of the contract.

- The merge of a team run records the Done to send (`team_merged_done`, `crates/store/src/project_db/team_done.rs`),
  in the merge's transaction, whichever way the merge was made (task page, Review page, `merge_run`, merge queue).
- The holder's desktop sends it in the shared projects' round, after the run's reports and on every run change
  (`crates/workspace/src/team_work/done.rs`, `sharing/project/team_reports.rs`): it reads the task from the server and
  sends the patch of *Mark as done* through the dispatcher on the revision read. The server releases the task's claim.
  A task Done already is settled without a patch; one Canceled meanwhile keeps the server's status. A server that
  cannot be reached keeps it waiting; a changed revision, or a close refused while a report of the run still waits,
  goes again at the next round.
- A refusal (`not_holder` once the claim was taken over, `not_admin`, …) is logged, kept with the run and never sent
  again; `get_run` reads it (`team.done`).
- Tests: `crates/workspace/tests/team_work/done.rs` (Done after the merge, an outage that keeps it waiting, a task Done
  already, the refusal kept).

## Left

- The task page does not show a refused Done yet: its line needs a design (and the design verification).
- The server records the Done as the holder's own (`closed.how = person`); a kind of its own for automation is a change
  of the contract.
- A team run git found merged outside Orchester asks for no Done (the old sharing stream's `record_merged_done` skips
  team runs).
