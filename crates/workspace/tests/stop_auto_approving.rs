//! The status bar's *Stop* for runs auto-approving: it stops every run that auto-approves when it writes,
//! not only the runs the bar showed when it was read.

mod support;

use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{data::ProjectTasks, run_approvals::stop_auto_approving_in};
use support::{Fixture, at};

#[test]
fn stop_auto_approving_stops_a_run_that_began_auto_approving_after_the_bar_was_read() {
  let mut fixture = Fixture::new();
  let (first, second) = (fixture.task("Add login", at(10)), fixture.task("Add logout", at(11)));
  let shown = fixture.started_run(first, at(20));
  let later = fixture.started_run(second, at(21));
  fixture
    .db
    .set_run_auto_approve(shown.id(), Some(true))
    .expect("auto-approve");
  let bar = ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("the bar reads the tasks");
  assert_eq!(
    bar.auto_approving.iter().map(|run| run.run).collect::<Vec<_>>(),
    [shown.id()]
  );
  // Another window turns auto-approval on for the second run before *Stop* is pressed.
  fixture
    .db
    .set_run_auto_approve(later.id(), Some(true))
    .expect("auto-approve");

  let mut stopped = stop_auto_approving_in(fixture.repo.path()).expect("stopped");
  stopped.sort();
  let mut both = vec![shown.id(), later.id()];
  both.sort();
  assert_eq!(stopped, both);
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  for run in [shown.id(), later.id()] {
    assert_eq!(db.run_auto_approve(run).expect("read"), Some(false), "{run}");
  }
}
