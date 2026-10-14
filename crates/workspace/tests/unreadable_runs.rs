//! A run whose record cannot be read back (edited or damaged outside Orchester): the task list,
//! its counts and the task page still load, show that run as unreadable, and count it nowhere.

mod support;

use orchester_core::{id::RunId, run::TaskPhase};
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::{
  data::ProjectTasks,
  detail::TaskDetail,
  list::{self, Latest, TaskTab},
  overview::{OverviewTab, Pulse, activity_empty, footer, recent_runs},
};
use rusqlite::Connection;
use support::{Fixture, at};

/// Sets `assignments` on the row of `run`, as an edit or damage outside Orchester would.
fn tamper(fixture: &Fixture, run: RunId, assignments: &str) {
  Connection::open(fixture.repo.path().join(".orchester/orchester.db"))
    .expect("open raw database")
    .execute(
      &format!("UPDATE runs SET {assignments} WHERE id = ?1"),
      [run.to_string()],
    )
    .expect("tamper");
}

#[test]
fn unreadable_latest_run_keeps_the_task_list_and_its_counts() {
  let mut fixture = Fixture::new();
  let first = fixture.task("Add login", at(10));
  let second = fixture.task("Fix zoom", at(11));
  let mut earlier = fixture.started_run(first, at(20));
  fixture.succeed(&mut earlier, at(30));
  let bad = fixture.started_run(first, at(40));
  fixture.started_run(second, at(50));
  // A started run has an attempt; a pending one never has.
  tamper(&fixture, bad.id(), "status = 'pending'");

  let loaded =
    ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("one unreadable run does not fail the load");
  let Latest::Unreadable(unreadable) = &loaded.phases[&first] else {
    panic!("the latest run of the task is unreadable");
  };
  assert_eq!((unreadable.id, unreadable.number), (bad.id(), Some(bad.number())));
  assert_eq!(unreadable.detail.key, "runs.unreadable.detail.invalid");
  let rows = list::rows(&loaded.tasks, &loaded, TaskTab::All);
  let row = rows.iter().find(|row| row.id == first).expect("the task's row");
  assert_eq!(row.phase, None);
  assert_eq!(row.detail.key, "runs.unreadable.title");
  assert_eq!(row.detail.args, [("run", bad.number().get().to_string())]);

  // Counts leave the unreadable run out; the task stays in All.
  assert_eq!(loaded.runs.values().sum::<usize>(), 2);
  assert_eq!(OverviewTab::All.count(&loaded.tasks, &loaded.phases), 2);
  assert_eq!(OverviewTab::Running.count(&loaded.tasks, &loaded.phases), 1);
  assert_eq!(list::queued(&loaded.tasks, &loaded.phases), 0);
  assert_eq!(Pulse::of(&loaded.phases, at(0)).running, 1);
  let footer = footer(&loaded.tasks, &loaded.phases);
  assert_eq!(footer.args, [("active", "1".to_owned()), ("completed", "0".to_owned())]);
  assert_eq!(loaded.without_runner().lost_to_runner, 1);
}

#[test]
fn unreadable_latest_run_shows_on_its_task_page_with_the_task_runs() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut cancelled = fixture.started_run(task, at(15));
  let events = cancelled.cancel(at(16)).expect("cancel");
  fixture.record(&cancelled, &events);
  let mut earlier = fixture.started_run(task, at(20));
  fixture.succeed(&mut earlier, at(30));
  let bad = fixture.started_run(task, at(40));
  tamper(&fixture, bad.id(), "snapshot = 'not json'");

  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("the task page reads");
  let unreadable = detail.latest_unreadable.as_ref().expect("its latest run is unreadable");
  assert_eq!((unreadable.id, unreadable.number), (bad.id(), Some(bad.number())));
  assert_eq!(unreadable.actions().args, [("run", bad.number().get().to_string())]);
  assert!(detail.run.is_none(), "no other run takes the latest run's place");
  let [latest, succeeded, first] = detail.history.as_slice() else {
    panic!("the runs are listed, newest first: {:?}", detail.history);
  };
  assert_eq!(
    (latest.id, latest.number, latest.phase),
    (bad.id(), Some(bad.number()), None)
  );
  assert_eq!(latest.detail.key, "runs.unreadable.title");
  assert!(latest.run.is_none() && latest.without_live_work().is_none());
  assert_eq!(
    (succeeded.number, succeeded.phase, succeeded.at),
    (Some(earlier.number()), Some(TaskPhase::Succeeded), Some(at(30)))
  );
  assert_eq!(succeeded.detail.key, "runs.history.notMerged");
  assert_eq!(
    (first.number, first.phase),
    (Some(cancelled.number()), Some(TaskPhase::Cancelled))
  );
  assert_eq!(
    first.detail.key, "runs.history.cancelledAt",
    "at the step it had not passed"
  );
  assert!(first.without_live_work().is_some(), "its worktree may be removed");
  assert!(detail.earlier_unreadable.is_empty());
}

#[test]
fn unreadable_earlier_run_is_named_on_the_page_of_the_latest_run() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let bad = fixture.started_run(task, at(20));
  let latest = fixture.started_run(task, at(40));
  tamper(&fixture, bad.id(), "snapshot = 'not json'");

  let loaded = ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("the task list reads");
  assert_eq!(loaded.phases[&task].phase(), Some(TaskPhase::Running));
  assert_eq!(loaded.runs.values().sum::<usize>(), 1);
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("the task page reads");
  assert_eq!(detail.run.as_ref().map(|run| run.id()), Some(latest.id()));
  assert!(detail.latest_unreadable.is_none());
  let ids: Vec<RunId> = detail.earlier_unreadable.iter().map(|run| run.id).collect();
  assert_eq!(ids, [bad.id()]);
}

/// Recent activity lists no run whose last change is unknown; with nothing else to list it says
/// why instead of claiming the project has no runs, though the task has an earlier one.
#[test]
fn unreadable_latest_run_is_not_reported_as_no_runs_in_recent_activity() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut earlier = fixture.started_run(task, at(20));
  fixture.succeed(&mut earlier, at(30));
  let bad = fixture.started_run(task, at(40));
  tamper(&fixture, bad.id(), "status = 'pending'");

  let loaded =
    ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("one unreadable run does not fail the load");
  assert!(recent_runs(&loaded.tasks, &loaded.phases, 3).is_empty());
  assert_eq!(activity_empty(&loaded.phases).key, "overview.activity.unreadable.one");

  let other = fixture.task("Fix zoom", at(50));
  let bad = fixture.started_run(other, at(60));
  tamper(&fixture, bad.id(), "status = 'pending'");
  let loaded =
    ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("unreadable runs do not fail the load");
  let empty = activity_empty(&loaded.phases);
  assert_eq!(empty.key, "overview.activity.unreadable.other");
  assert_eq!(empty.args, [("count", "2".to_owned())]);
}
