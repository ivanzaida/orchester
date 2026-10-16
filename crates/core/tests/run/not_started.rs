//! Before a run starts: the free branch it moves to when a branch of its own name exists, and failing it when its
//! worktree cannot be created.

use orchester_core::run::{BranchName, Run, RunError, RunFailure, RunStatus};

use crate::fixture::{Logged, at, stored};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("branch")
}

#[test]
fn a_pending_run_moves_to_a_free_branch_and_replays_on_it() {
  let mut logged = Logged::pending();
  let planned = logged.run.branch().clone();
  let free = branch(&format!("{planned}-2"));

  logged.apply(|run| run.choose_branch(free.clone(), at(2)));
  logged.apply(|run| run.start(at(3)));

  assert_eq!(logged.run.branch(), &free);
  let kinds: Vec<_> = logged.log.iter().map(|event| event.kind().name()).collect();
  assert_eq!(kinds, ["created", "branch_chosen", "started", "attempt_started"]);
  assert_eq!(Run::replay(&logged.log).expect("replay"), logged.run);
  assert_eq!(Run::restore(stored(&logged.run)).expect("restore"), logged.run);
}

#[test]
fn a_run_keeps_its_branch_unless_another_was_chosen_before_it_started() {
  let mut pending = Logged::pending();
  let own = pending.run.branch().clone();
  assert_eq!(
    pending.run.choose_branch(own, at(2)),
    Ok(Vec::new()),
    "its own name changes nothing"
  );

  let mut started = Logged::started();
  let free = branch(&format!("{}-2", started.run.branch()));
  assert!(matches!(
    started.run.choose_branch(free, at(12)),
    Err(RunError::InvalidRunTransition {
      status: RunStatus::Running,
      event: "branch_chosen"
    })
  ));
}

#[test]
fn a_run_whose_worktree_was_not_created_fails_before_it_started_and_runs_again() {
  let mut logged = Logged::pending();
  let reason = "`git worktree add` failed (Some(128)): fatal: disk full".to_owned();

  logged.apply(|run| run.fail_to_start(reason.clone(), at(2)));

  let failed = RunStatus::Failed(RunFailure::WorktreeNotCreated(reason.clone()));
  assert_eq!(logged.run.status(), &failed);
  assert!(logged.run.attempts().is_empty(), "no step ran");
  assert!(logged.run.recovery_step(None).is_none(), "no worktree to resume in");
  assert!(logged.run.status().can_run_again(), "a new run may start");
  assert_eq!(Run::replay(&logged.log).expect("replay"), logged.run);
  assert_eq!(Run::restore(stored(&logged.run)).expect("restore"), logged.run);
}

#[test]
fn only_a_pending_run_fails_for_its_worktree() {
  let mut started = Logged::started();
  assert!(matches!(
    started.run.fail_to_start("no worktree".into(), at(12)),
    Err(RunError::InvalidRunTransition { event: "failed", .. })
  ));

  let mut restored = stored(&Logged::started().run);
  restored.status = RunStatus::Failed(RunFailure::WorktreeNotCreated("no worktree".into()));
  assert!(matches!(Run::restore(restored), Err(RunError::Inconsistent(_))));
}
