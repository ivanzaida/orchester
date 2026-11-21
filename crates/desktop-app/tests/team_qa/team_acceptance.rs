//! A team module's *Delivery* on its Summary (`docs/plan/orchestrator-autonomy/OA-32-…`), on the QA app's
//! `team-member` state: the acceptance pipeline and *Deliver when accepted* a member chooses on this computer, read as
//! this computer keeps them, a change kept at once, and no *Its own branch* switch, since the team assigns the branch.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_acceptance`

use orchester_app::{
  routes::{ModuleTab, ProjectPage, Route},
  test_support::project_id,
};
use orchester_core::{
  id::{ModuleId, ResourceId},
  module::delivery::AcceptanceChoice,
};
use orchester_store::ProjectDb;

use super::{prepared, team::State};

/// Webhooks v2, the first of the modules the QA app's server sets.
const WEBHOOKS: &str = "0191c7c0-0000-7000-8000-000000200000";

#[test]
fn team_qa_acceptance_a_members_module_keeps_its_choice_on_this_computer() {
  let (mut window, _server) = prepared(State::Member);
  let module: ModuleId = WEBHOOKS.parse().expect("a module id");
  let root = window.data.repo("repo-kontur");
  let chosen = AcceptanceChoice {
    acceptance: Some(ResourceId::generate()),
    deliver_when_accepted: true,
  };
  ProjectDb::open(&root)
    .expect("the project")
    .set_team_module_acceptance(module, chosen)
    .expect("chosen here");

  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Module(module, ModuleTab::Summary),
  ));
  // The choice is read off the UI thread: the rows show it once read.
  window.until("the choice kept here", |window| {
    window
      .texts("module-deliver-when-accepted-row-description")
      .is_some_and(|hint| hint.join(" ").starts_with("The Orchestrator"))
  });
  assert!(
    window.texts("module-own-branch").is_none(),
    "the team assigns the branch"
  );
  assert_eq!(
    window.text("module-delivery-foot-text"),
    "Kept on this computer: the team's server keeps neither, so each of you chooses them."
  );
  assert_eq!(
    window.text("module-deliver-when-accepted-row-description"),
    "The Orchestrator asks the team's server to deliver the branch when acceptance finds nothing; an Admin or the \
     automatic merge decides"
  );

  window.pass();
  let read = ProjectDb::open(&root).and_then(|db| db.team_module_acceptance(module));
  assert_eq!(read.expect("read"), chosen, "reading the choice writes nothing");

  // Turning Deliver when accepted off is kept here at once.
  window.press("module-deliver-when-accepted");
  window.until("the choice to be kept", |_| {
    ProjectDb::open(&root)
      .and_then(|db| db.team_module_acceptance(module))
      .is_ok_and(|kept| kept.acceptance == chosen.acceptance && !kept.deliver_when_accepted)
  });
}
