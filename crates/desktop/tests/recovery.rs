//! The runs the start-up recovery (02.E `o8DCd`) lists after a crash, read from a registered
//! project's database: a run left at work is interrupted; a run that waits for a person's answer
//! or decision keeps waiting, as the runner leaves it when it attaches.

mod support;

use orchester_core::run::WaitReason;
use orchester_desktop::{programs::Programs, recovery::interrupted_runs, workspace::Workspace};
use orchester_ipc::SessionEnd;
use orchester_store::AppDb;
use support::{Fixture, at};

#[test]
fn recovery_lists_a_run_at_work_but_not_one_waiting_for_a_person() {
  let mut fixture = Fixture::gated();
  let asking = fixture.task("Add login", at(10));
  let mut question = fixture.started_run(asking, at(20));
  let execution = question.current_attempt().expect("attempt").executions()[0].id();
  let events = question
    .wait_step(execution, WaitReason::Message, at(30))
    .expect("the step waits for an answer");
  fixture.record(&question, &events);
  let planning = fixture.task("Plan zoom", at(40));
  let mut plan = fixture.started_run(planning, at(50));
  fixture.propose(&mut plan, "1. Zoom.", at(60));
  let working = fixture.task("Write docs", at(70));
  let at_work = fixture.started_run(working, at(80));

  let app = tempfile::tempdir().expect("an app folder");
  let workspace = Workspace::new(AppDb::open(&app.path().join("orchester.db")).expect("the app database"));
  workspace.open(fixture.repo.path()).expect("the project is registered");
  let listed = interrupted_runs(&workspace, &Programs::processes(), SessionEnd::Crashed, at(90)).expect("read");
  let runs: Vec<_> = listed.iter().map(|run| run.run).collect();
  assert_eq!(runs, [at_work.id()]);
}
