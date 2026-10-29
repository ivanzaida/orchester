//! The Summary's *Raise the limit* under Autopilot (ORC-348): an MCP client's press of *Continue with …* is refused
//! and the run stays stopped until the person lets automation answer approvals; then it continues, as today.
use orchester_app::routes::TaskView;
use orchester_core::{
  id::RunId,
  run::{Answerer, PauseReason, RunLimit, RunStatus},
  store::RunStore,
};
use orchester_store::ProjectDb;

use super::{
  super::{decisions_reach_person::autopilot, tools_window::text},
  Fixture,
};

fn at_token_limit(db: &ProjectDb, run: RunId) -> bool {
  db.run(run)
    .expect("run")
    .is_some_and(|run| *run.status() == RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens)))
}

#[test]
fn decisions_reach_person_limit_card_refuses_client_input_until_the_person_lets_automation_answer() {
  let mut f = Fixture::new(
    "task-states",
    "Explain an expired GitHub token in the status bar",
    TaskView::Summary,
  );
  autopilot(&f.repo);
  let (mut db, run) = (f.db(), f.run);
  // The seed spent the project's day: without room in it, the runner would refuse to go on whoever raised the limit.
  db.update_project_limits(|limits| Some(limits.with_daily_budget_limit(None)))
    .expect("no daily budget");
  assert!(at_token_limit(&db, run), "the seed stops the run at its token limit");
  f.app.until("limit card online", |app| {
    app.shows("raise-limit-continue") && !app.shows("raise-limit-offline")
  });
  f.press("raise-limit-continue", Answerer::Automation);
  f.app
    .until("client raise refused", |app| app.shows("raise-limit-person-only"));
  assert!(at_token_limit(&db, run), "nothing raised");
  f.consent();
  f.press("raise-limit-continue", Answerer::Automation);
  f.app.until("the run continues with the raised limit", |app| {
    assert!(
      !app.shows("raise-limit-refused"),
      "the runner refused the raise: {}",
      text(app, "raise-limit-refused")
    );
    !at_token_limit(&db, run)
  });
}
