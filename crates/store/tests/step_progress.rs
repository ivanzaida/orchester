//! The progress line an agent reports for its step (`report_progress`): one per step execution, the latest replacing
//! the one before.

mod support;

use orchester_core::{id::StepExecutionId, run::StepProgress};
use support::{at, started_run};

#[test]
fn step_progress_keeps_the_latest_line() {
  let (_repo, mut db, _run, execution) = started_run();
  assert_eq!(db.step_progress(execution).expect("read"), None);
  let line = |text: &str| StepProgress::new(text).expect("a line");
  db.report_step_progress(execution, &line("reading the sources"), at(10))
    .expect("kept");
  db.report_step_progress(execution, &line("running the tests"), at(20))
    .expect("replaced");
  assert_eq!(
    db.step_progress(execution).expect("read"),
    Some((line("running the tests"), at(20)))
  );
  assert!(
    db.report_step_progress(StepExecutionId::generate(), &line("x"), at(30))
      .is_err(),
    "only a step execution of the project"
  );
}
