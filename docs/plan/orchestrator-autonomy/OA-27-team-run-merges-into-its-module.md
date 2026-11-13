# OA-27 · A team task's run merges into its module's branch

- **Status:** done on master, outside this plan (`b4abf405d`, merged here with `origin/master` on 2026-11-13)
- **Kind:** core + workspace
- **Depends on:** OA-16
- **Branch:** —
- **Tokens:** —

## Goal

The work of a team task's run reaches the branch its module is delivered from, by the paths a local run's does:
`merge_run`, the task page, the merge queue and the Orchestrator's own merge.

## Today

Every merge goes through `merge_in_as` (`crates/workspace/src/tasks/integration.rs`), which refuses a run whose
route is not local (`refuse_team_run`: "delivered with its module, not merged from here"). The merge queue and the
review's approve refuse the same way. So a team run's branch, made from its module's branch, is never merged into
it, while the contract says it is (D3, `docs/cloud-api.md`), and a delivery would carry the module's branch without
the run's work. `merge_run` of a team run asks for the module's *delivery* instead
(`app_mcp/tools/setup/merge.rs`, `team/deliver.rs`).

## Change

- A team run merges into its module's branch as a local run merges into a base branch nobody has checked out
  (`merge/beside.rs`, OA-16): a temporary worktree and a compare-and-swap of the ref, never the person's working
  tree. Under the claim of the task, checked as a start checks it.
- `merge_run` of a team run merges the run; the module's delivery gets a call of its own (OA-32). The merge queue,
  the review's approve and the task page follow.
- The server is told what it is told today of a run's work (`work.report`); a new report, if one is needed for the
  merge, is a change of the contract and is said in this task before it is built.
- Conflicts go to an agent (`resolve_conflicts`), as for a local run beside the checkout (the gap OA-16 names).

## Edge cases

- The claim was lost before the merge: refused `claim_fenced`, as a start is.
- The module's branch is checked out in some worktree: refused, as `beside` refuses (`CheckedOutIn`).
- A run of the old sharing stream (no team record) already merges as a local run: unchanged.

## Done when

- Workspace tests with the stand-in server (`orchester-api`'s `test-support`): a team run merged into its module's
  branch by each path, the claim lost, conflicts.
- The row *A run's work reaches its module's branch* leaves `docs/design/local-and-team-parity.md`.
- `tools/check.ps1` passes.

## Done on master (2026-11-13)

`b4abf405d` *a team task's run merges on this computer, with or without a module* did this while the plan was being
written: `refuse_team_run` lets a recorded team run through (`crates/workspace/src/tasks/integration.rs`), `merge_run`
merges a team run into its branch and then asks for its module's delivery, the task page has *Merge and deliver*,
and a team task in no module starts and lands on the checked-out branch. Not checked by this plan's author beyond
reading that commit and `refuse_team_run`; what is left for the Orchestrator (its own merge in a turn it started,
which `auto_merge` still leaves to the person) is OA-29's.
