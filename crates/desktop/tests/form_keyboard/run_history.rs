//! *Runs of this task* and *Remove worktree* for every ended run (ORC-63), headlessly, on the real
//! runner, `orchester-fake-codex` and git: an older failed run's worktree with an uncommitted file
//! is removed from its row only once discarding is agreed to; an interrupted run's dialog says it
//! can't be continued afterwards, after which Continue run is unavailable and Run again starts the
//! next run; a run at work offers no removal; one removal runs at a time per task; and the rows keep
//! their columns at 1440 and stack at 960.

use std::{fs, path::PathBuf, sync::atomic::Ordering};

use orchester_core::{
  run::{InterruptCause, Run, RunStatus},
  store::{ActivityStore, RunStore, WorktreeStore},
};
use orchester_desktop::programs::Programs;
use orchester_store::ProjectDb;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  integration::blocked_git,
  task_run::{create_and_run, has, init_repository, latest_run, programs, seed},
};

fn holds(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  desktop
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| contains_text(element, text))
}

fn worktree_of(desktop: &Desktop, run: &Run) -> PathBuf {
  desktop
    .repo()
    .join(".orchester")
    .join("worktrees")
    .join(run.id().to_string())
}

/// Whether the agent of the latest run reported its CLI session, which continuing would resume.
fn session_recorded(desktop: &Desktop) -> bool {
  let Some(run) = latest_run(desktop.repo()) else {
    return false;
  };
  ProjectDb::open(desktop.repo())
    .expect("open")
    .invocations(run.id())
    .expect("invocations")
    .iter()
    .any(|invocation| invocation.session.is_some())
}

fn row(run: &Run) -> String {
  format!("run-row-{}", run.id())
}

/// A task whose run #1 failed on the fake Codex and was run again, keeping its worktree, as run
/// #2, which failed too; the task page shows run #2.
fn failed_twice(program_set: Programs) -> (Desktop, Run) {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_programs(runner, program_set, init_repository, |repo| {
    seed(repo, &fake, false);
  });
  create_and_run(&mut desktop, "Report failure.", "1400000");
  desktop.wait_until("run #1 failed", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status().can_run_again()) && has(desktop, "task-run-again")
  });
  let first = latest_run(desktop.repo()).expect("run #1");
  desktop.click("task-run-again");
  desktop.wait_for("run-again-dialog");
  desktop.click("run-again-confirm");
  desktop.wait_until("run #2 failed", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.number().get() == 2 && run.status().can_run_again())
      && has(desktop, "task-run-again")
  });
  (desktop, first)
}

/// The task page read again, which reads what the runs left anew.
fn reopen_task(desktop: &mut Desktop) {
  desktop.click("task-breadcrumb");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-runs");
}

#[test]
fn run_history_removes_an_older_failed_run_worktree_with_its_uncommitted_file_once_agreed() {
  let (mut desktop, first) = failed_twice(Programs::processes());
  let old = worktree_of(&desktop, &first);
  fs::write(old.join("notes.txt"), "not committed\n").expect("an uncommitted file");
  reopen_task(&mut desktop);
  let first_row = row(&first);
  desktop.wait_until("run #1's worktree line", |desktop| {
    holds(
      desktop,
      &first_row,
      "Worktree · 1 file not committed · orchester/kon-1-run-1",
    )
  });
  assert!(shows(&mut desktop, "task-runs", "Runs of this task"));
  assert!(shows(&mut desktop, &first_row, "Failed at Implement"));
  let latest = latest_run(desktop.repo()).expect("run #2");
  assert!(shows(&mut desktop, &row(&latest), "this run"));
  assert!(!holds(&mut desktop, &first_row, "this run"));

  desktop.click("run-remove-worktree-1");
  desktop.wait_for("remove-worktree-dialog");
  assert!(shows(
    &mut desktop,
    "remove-worktree-dialog",
    "Remove the worktree of run #1?"
  ));
  assert!(shows(&mut desktop, "remove-worktree-list", "notes.txt"));
  assert!(shows(&mut desktop, "remove-worktree-list", "1 file not committed"));
  assert!(
    shows(&mut desktop, "remove-worktree-list", "kept · not merged"),
    "its branch is not merged anywhere"
  );
  desktop.click("remove-worktree-confirm");
  assert!(old.join("notes.txt").is_file(), "not before discarding is agreed to");
  desktop.click("remove-worktree-discard");
  desktop.click("remove-worktree-confirm");
  desktop.wait_until("the removal", |desktop| {
    !has(desktop, "remove-worktree-dialog") && holds(desktop, &first_row, "Worktree removed")
  });
  assert!(!old.exists());
  assert!(shows(&mut desktop, &first_row, "orchester/kon-1-run-1 kept"));
  assert!(
    !has(&mut desktop, "run-remove-worktree-1"),
    "a removed worktree has no action"
  );
  assert!(worktree_of(&desktop, &latest).is_dir(), "run #2 keeps its worktree");
  assert!(
    has(&mut desktop, "task-remove-worktree"),
    "the latest run keeps its topbar button"
  );
  let db = ProjectDb::open(desktop.repo()).expect("open");
  assert!(db.worktree_removal(first.id()).expect("read").is_some());
}

#[test]
fn run_history_buttons_wait_while_another_run_of_the_task_is_removed() {
  let (program_set, gate) = blocked_git(&["worktree", "remove"]);
  let (mut desktop, first) = failed_twice(program_set);
  desktop.wait_until("run #1's button", |desktop| has(desktop, "run-remove-worktree-1"));
  desktop.click("run-remove-worktree-1");
  desktop.wait_for("remove-worktree-dialog");
  desktop.click("remove-worktree-confirm");
  let first_row = row(&first);
  desktop.wait_until("git removing run #1's worktree", |desktop| {
    gate.entered.load(Ordering::SeqCst) && holds(desktop, &first_row, "Removing…")
  });
  desktop.click("remove-worktree-cancel");
  assert!(has(&mut desktop, "remove-worktree-dialog"), "Cancel waits for git");
  // Behind the dialog, run #2's row and the topbar wait; pressing them changes nothing.
  for id in ["run-remove-worktree-2", "task-remove-worktree"] {
    desktop.click(id);
    assert!(
      shows(&mut desktop, "remove-worktree-dialog", "Remove the worktree of run #1?"),
      "{id} opened another removal"
    );
  }
  gate.release();
  desktop.wait_until("the removal", |desktop| {
    !has(desktop, "remove-worktree-dialog") && holds(desktop, &first_row, "Worktree removed")
  });
  assert!(!worktree_of(&desktop, &first).exists());
  assert!(worktree_of(&desktop, &latest_run(desktop.repo()).expect("run #2")).is_dir());
  // With the removal done, run #2's is offered again.
  desktop.click("run-remove-worktree-2");
  desktop.wait_for("remove-worktree-dialog");
  assert!(shows(
    &mut desktop,
    "remove-worktree-dialog",
    "Remove the worktree of run #2?"
  ));
}

#[test]
fn run_history_offers_no_removal_while_a_run_is_at_work() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("the run", |desktop| latest_run(desktop.repo()).is_some());
  let run = latest_run(desktop.repo()).expect("run #1");
  let run_row = row(&run);
  desktop.wait_until("the running run's row", |desktop| {
    holds(
      desktop,
      &run_row,
      "Worktree in use · Remove worktree comes back when the run ends",
    )
  });
  assert!(shows(&mut desktop, &run_row, "this run"));
  assert!(!has(&mut desktop, "run-remove-worktree-1"));
  assert!(!has(&mut desktop, "task-remove-worktree"));
  desktop.click("task-cancel-run");
  desktop.wait_until("the cancelled run's Remove worktree", |desktop| {
    has(desktop, "run-remove-worktree-1") && has(desktop, "task-remove-worktree")
  });
}

#[test]
fn run_history_an_interrupted_run_without_its_worktree_cannot_continue_and_runs_again() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("the agent at work", |desktop| session_recorded(desktop));
  let mut desktop = desktop.crash_and_restart(|| {});
  desktop.wait_until("the recovery", |desktop| has(desktop, "recovery-dialog"));
  desktop.click("recovery-dont-continue");
  desktop.wait_until("the interrupted run", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Interrupted(InterruptCause::Crashed))
  });
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_until("Remove worktree beside Continue run", |desktop| {
    has(desktop, "task-remove-worktree") && has(desktop, "task-continue-run") && has(desktop, "run-remove-worktree-1")
  });
  let run = latest_run(desktop.repo()).expect("run #1");
  let worktree = worktree_of(&desktop, &run);

  desktop.click("task-remove-worktree");
  desktop.wait_for("remove-worktree-interrupted");
  assert!(shows(
    &mut desktop,
    "remove-worktree-interrupted",
    "Run #1 can’t be continued afterwards"
  ));
  assert!(shows(
    &mut desktop,
    "remove-worktree-interrupted",
    "Without it the run stays interrupted for good; Run again starts a new run instead."
  ));
  if has(&mut desktop, "remove-worktree-discard") {
    desktop.click("remove-worktree-discard");
  }
  desktop.click("remove-worktree-confirm");
  desktop.wait_until("the run that can't continue", |desktop| {
    !has(desktop, "remove-worktree-dialog") && has(desktop, "task-continue-unavailable")
  });
  assert!(!worktree.exists());
  assert!(shows(
    &mut desktop,
    "task-continue-unavailable",
    "Continue run is unavailable: run #1’s worktree was removed."
  ));
  assert!(shows(&mut desktop, "task-notice", "Run #1 can’t continue"));
  assert!(shows(
    &mut desktop,
    "task-progress-steps",
    "Interrupted · can’t continue"
  ));
  assert!(shows(&mut desktop, &row(&run), "can’t continue"));
  assert!(!has(&mut desktop, "task-remove-worktree"));
  // Continue run stays, unavailable: pressing it asks the runner nothing.
  desktop.click("task-continue-run");
  assert_eq!(latest_run(desktop.repo()).expect("run").status(), run.status());

  desktop.wait_for("task-run-again");
  desktop.click("task-run-again");
  desktop.wait_for("run-again-dialog");
  desktop.click("run-again-confirm");
  desktop.wait_until("run #2", |desktop| {
    !has(desktop, "run-again-dialog") && latest_run(desktop.repo()).is_some_and(|run| run.number().get() == 2)
  });
  let db = ProjectDb::open(desktop.repo()).expect("open");
  assert_eq!(
    db.run(run.id()).expect("read").expect("run #1").status(),
    &RunStatus::Interrupted(InterruptCause::Crashed),
    "run #1 stays interrupted in its record"
  );
  desktop.click("task-cancel-run");
}

#[test]
fn run_history_rows_keep_their_columns_at_1440_and_stack_at_960() {
  let (mut desktop, first) = failed_twice(Programs::processes());
  let first_row = row(&first);
  desktop.wait_until("run #1's button", |desktop| has(desktop, "run-remove-worktree-1"));
  let (wide_row, wide_button) = (desktop.bounds(&first_row), desktop.bounds("run-remove-worktree-1"));
  assert!(
    wide_button.x > wide_row.x + wide_row.width * 0.7,
    "{wide_button:?} sits at the right of {wide_row:?}"
  );
  assert!(wide_button.x + wide_button.width <= wide_row.x + wide_row.width + 0.5);

  desktop.tree.resize(960, 960);
  desktop.wait_until("the stacked row", |desktop| {
    desktop.bounds(&first_row).height > wide_row.height + 8.0
  });
  let (row_bounds, button) = (desktop.bounds(&first_row), desktop.bounds("run-remove-worktree-1"));
  assert!(
    button.y < row_bounds.y + 40.0,
    "{button:?} stays on the run's line of {row_bounds:?}"
  );
  assert!(button.x + button.width <= row_bounds.x + row_bounds.width + 0.5);
  assert!(button.x > row_bounds.x + row_bounds.width * 0.5);
}
