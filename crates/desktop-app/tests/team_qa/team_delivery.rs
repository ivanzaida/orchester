//! A team project's Usage › Delivery (`docs/plan/orchestrator-autonomy/OA-36-…`), on the QA app's `team-member` state:
//! the figures built from the tasks and modules the stand-in team server holds, with what they count said under the
//! toolbar, each open module's forecast, and no cumulative flow, which the card says the team's server keeps no history
//! for.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_delivery`

use orchester_app::{
  routes::{ProjectPage, Route},
  test_support::project_id,
};
use orchester_workspace::delivery::DeliveryPeriod;

use super::{prepared, team::State};

#[test]
fn team_qa_delivery_of_a_team_project_is_built_from_its_servers_tasks() {
  let (mut window, _server) = prepared(State::Member);
  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Delivery(DeliveryPeriod::Month),
  ));
  window.until("the team's figures", |window| {
    window.texts("delivery-flow-team").is_some()
  });
  assert!(
    window
      .text("delivery-team-counts")
      .starts_with("Built from the team's tasks on its server"),
    "what the figures count is said"
  );
  assert!(
    window
      .text("delivery-flow-team")
      .contains("The team's server keeps no history of a task's workflow status."),
    "no flow, and why"
  );
  let forecasts = window.text("delivery-forecast");
  assert!(
    forecasts.contains("Webhooks v2"),
    "the team's open modules: {forecasts}"
  );
}
