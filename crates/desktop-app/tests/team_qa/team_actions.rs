//! A team project's actions on its pages (ORC-732, `Team authority · redesign` `BZc66` rows 2–5 and 7), on the QA
//! app's `team-claims`, `team-module-work`, `team-admin-work` and `team-version-refused` states with the stand-in
//! team server's commands: the Task page's claim line in each state the server reads, the run the server reports of
//! a task another computer holds and its claim's property, Claim of a task nobody holds, Take over of a stale claim
//! asked first and refused when its holder renewed it first; the Module page's take over refused while a task of it
//! runs; an Admin's branch change refused for a taken branch, for running work and for someone else's change first;
//! Automatic merge changed by someone else first, and its value for a member; and the refusal of this version of
//! Orchester.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_actions`

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{ModuleTab, ProjectPage, ProjectSettingsTab, Route, TaskView},
  test_support::{Window, project_id},
};
use orchester_core::{id::ModuleId, project::TaskKey};

use super::{prepared, team::State};

/// Webhooks v2, the first of the modules the QA app's server sets.
const WEBHOOKS: &str = "0191c7c0-0000-7000-8000-000000200000";

fn task_route(key: &str) -> Route {
  let key: TaskKey = key.parse().expect("a task key");
  Route::Project(project_id(), ProjectPage::task(key, TaskView::Summary, false))
}

fn webhooks() -> Route {
  let module: ModuleId = WEBHOOKS.parse().expect("a module id");
  Route::Project(project_id(), ProjectPage::Module(module, ModuleTab::Summary))
}

fn sharing() -> Route {
  Route::Project(project_id(), ProjectPage::Settings(ProjectSettingsTab::Sharing))
}

/// The window on `route` in `state`, once `shown` shows.
fn on(state: State, route: Route, shown: &str) -> (Window, TeamServer) {
  let (mut window, server) = prepared(state);
  go(&mut window, route, shown);
  (window, server)
}

fn go(window: &mut Window, route: Route, shown: &str) {
  window.navigation().go(route);
  window.until(shown, |window| window.texts(shown).is_some());
}

/// The claim line's title, once it says `starts`.
fn claim_title(window: &mut Window, starts: &str) -> String {
  window.until(starts, |window| {
    window
      .texts("task-team-claim-title")
      .is_some_and(|title| title.concat().starts_with(starts))
  });
  window.text("task-team-claim-title")
}

#[test]
fn team_qa_actions_the_claim_line_says_who_holds_each_task() {
  let (mut window, _server) = on(State::Claims, task_route("KON-13"), "task-team-claim-title");
  assert_eq!(claim_title(&mut window, "Claimed by"), "Claimed by mara");
  for (key, title) in [
    ("KON-10", "Claimed by you on another computer"),
    ("KON-19", "Claimed under module Webhooks v2 by olha"),
    ("KON-16", "mara's claim is stale"),
    ("KON-5", "Nobody holds KON-5"),
    ("KON-24", "Your claim is stale"),
  ] {
    go(&mut window, task_route(key), "task-team-claim-title");
    assert_eq!(claim_title(&mut window, title), title, "{key}");
  }
  assert!(window.texts("task-team-claim-take").is_none(), "KON-24 is the person's");
}

/// 02 `nMUwg`, 02b `XbNXU`, 02c `l82OS`, 02d `lpji3`: a task another computer holds has the run its server reports in
/// place of *No runs yet*, and nothing to start here.
#[test]
fn team_qa_actions_a_claim_held_elsewhere_shows_the_run_its_server_reports() {
  let (mut window, _server) = on(State::Claims, task_route("KON-13"), "task-event-text");
  for (key, meta, event) in [
    (
      "KON-13",
      "· on mara's computer",
      "stay on mara's computer; the team server only knows the run is Running",
    ),
    (
      "KON-10",
      "· on your other computer",
      "stay on your other computer; this computer sees what the team server",
    ),
    (
      "KON-19",
      "· not started",
      "No run yet. olha starts it under their module claim",
    ),
    (
      "KON-16",
      "· on mara's computer",
      "The team server last heard from mara's run at",
    ),
  ] {
    go(&mut window, task_route(key), "task-team-claim-title");
    window.until(event, |window| {
      window
        .texts("task-event-text")
        .is_some_and(|texts| texts.concat().contains(event))
    });
    let shown = window.text("task-run-meta");
    assert!(shown.ends_with(meta), "{key}: {shown}");
    assert!(window.texts("task-summary-empty").is_none(), "{key} has no empty state");
    assert!(window.texts("task-empty-start").is_none(), "{key} starts nothing here");
  }
}

/// 02b `XbNXU`, 03c `W6Ncf`: the person's claim another computer holds reads so in the properties, never as this
/// computer's. (The headless Kontur has none of the QA app's seeded runs, so a fenced run's stop is the QA app's.)
#[test]
fn team_qa_actions_the_persons_claim_on_another_computer_reads_so() {
  let (mut window, _server) = on(State::Claims, task_route("KON-23"), "task-property-claim-label");
  for key in ["KON-23", "KON-10"] {
    go(&mut window, task_route(key), "task-property-claim-label");
    window.until("the other computer", |window| {
      window.text("task-property-claim-label") == "You · other computer"
    });
  }
}

#[test]
fn team_qa_actions_claim_takes_a_task_nobody_holds() {
  let (mut window, _server) = on(State::Claims, task_route("KON-5"), "task-team-claim-take");
  window.press("task-team-claim-take");
  assert_eq!(claim_title(&mut window, "Claimed by you"), "Claimed by you");
  assert!(window.texts("task-team-refused").is_none());
}

#[test]
fn team_qa_actions_take_over_asks_first_and_says_the_holder_renewed_first() {
  let (mut window, _server) = on(State::Claims, task_route("KON-16"), "task-team-take-over");
  window.press("task-team-take-over");
  window.until("the question", |window| window.texts("task-take-over-title").is_some());
  assert_eq!(window.text("task-take-over-title"), "Take over KON-16 from mara?");
  window.press("task-take-over-confirm");
  window.until("the refusal", |window| window.texts("task-team-refused").is_some());
  let said = window.text("task-team-refused");
  assert!(said.contains("Couldn't take over KON-16"), "{said}");
  assert!(said.contains("mara renewed the claim at"), "{said}");
  assert_eq!(claim_title(&mut window, "Claimed by"), "Claimed by mara");
}

#[test]
fn team_qa_actions_a_module_take_over_is_refused_while_a_task_of_it_runs() {
  let (mut window, _server) = on(State::ModuleWork, webhooks(), "module-team-take-over");
  assert_eq!(window.text("module-team-claim-title"), "olha's module claim is stale");
  window.press("module-team-take-over");
  window.until("the refusal", |window| window.texts("module-team-refused").is_some());
  let said = window.text("module-team-refused");
  assert!(said.contains("Couldn't take over Webhooks v2"), "{said}");
  assert!(said.contains("is Running under olha's module claim"), "{said}");
}

#[test]
fn team_qa_actions_a_member_reads_the_branch_and_changes_nothing() {
  let (mut window, _server) = on(State::ModuleWork, webhooks(), "module-team-branch-title");
  assert_eq!(window.text("module-team-branch-title"), "Branch module/webhooks-v2");
  assert!(
    window.texts("module-team-branch-change").is_none(),
    "a member changes no branch"
  );
}

#[test]
fn team_qa_actions_an_admins_branch_change_meets_each_refusal() {
  let (mut window, _server) = on(State::AdminWork, webhooks(), "module-team-branch-change");
  let ask = |window: &mut Window| {
    window.press("module-team-branch-change");
    window.until("the question", |window| {
      window.texts("module-team-branch-dialog-title").is_some()
    });
    window.set("module-team-branch-field", "module/webhooks-retry");
    window.press("module-team-branch-confirm");
  };
  ask(&mut window);
  window.until("a taken branch", |window| {
    window
      .texts("module-team-branch-dialog")
      .is_some_and(|texts| texts.concat().contains("Another module already works on this branch"))
  });
  window.press("module-team-branch-confirm");
  window.until("running work", |window| window.texts("module-team-refused").is_some());
  assert!(
    window
      .text("module-team-refused")
      .contains("Can't change the branch while work runs")
  );
  ask(&mut window);
  window.until("someone else's change", |window| {
    window.texts("module-team-conflict").is_some()
  });
  let said = window.text("module-team-conflict");
  assert!(said.contains("olha changed the branch first"), "{said}");
  assert!(said.contains("It's module/webhooks-signing now"), "{said}");
  assert!(window.texts("module-team-conflict-reapply").is_some());
}

#[test]
fn team_qa_actions_automatic_merge_changed_first_shows_what_it_is_now() {
  let (mut window, _server) = on(State::AdminWork, sharing(), "sharing-policy-merge-switch");
  window.until("the threshold", |window| {
    window.text("sharing-policy-stale-value") == "15 min"
  });
  window.press("sharing-policy-merge-switch");
  window.until("someone else's change", |window| {
    window.texts("sharing-policy-conflict").is_some()
  });
  let said = window.text("sharing-policy-conflict");
  assert!(said.contains("Automatic merge was changed first"), "{said}");
  assert!(said.contains("It's On now; you turned it off."), "{said}");
}

#[test]
fn team_qa_actions_a_member_reads_automatic_merge_with_a_lock() {
  let (mut window, _server) = on(State::Member, sharing(), "sharing-policy-merge-value");
  assert_eq!(window.text("sharing-policy-merge-value"), "Off");
  assert!(
    window.texts("sharing-policy-merge-switch").is_none(),
    "a member switches nothing"
  );
}

#[test]
fn team_qa_actions_a_server_that_refuses_this_version_says_so() {
  let (mut window, _server) = on(State::VersionRefused, sharing(), "sharing-version-refused");
  assert!(
    window
      .text("sharing-version-refused")
      .contains("This team server needs another version of Orchester")
  );
}
