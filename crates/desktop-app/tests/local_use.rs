//! Local use without an Orchester Cloud account (the person's decision, 2026-10-28): a fresh install opens the
//! workspace, adds a project, creates a task and opens its pages with the cloud asked nothing; what needs a sign-in
//! says so in place and offers it; the tabs follow a sign-in and a sign-out; and a project that waits for its account
//! tells nothing meanwhile. Running a task to its end needs a real runner: the QA copy's `empty -FakeAgents` proves
//! that.
//!
//! `cargo test -p orchester-app --test local_use local_use_`

mod support;

use orchester_app::routes::{JoinWay, OrchesterView, ProjectPage, ProjectSettingsTab, Route, TaskList, TaskListTab};
use support::{
  Data, Window, add_project, project_id,
  signed_in::{self, account_cloud::EMAIL},
};

/// The window of a new data folder with the stand-in set as the cloud, nobody signed in.
fn fresh() -> Window {
  let data = Data::new();
  let launch = data.launch();
  let services = launch.services().expect("the session started").clone();
  data
    .runtime
    .block_on(services.cloud.set_address(Some(signed_in::cloud_address(&data.cloud))))
    .expect("a test cloud");
  Window::of(data, launch)
}

fn shows(window: &mut Window, id: &str) {
  window.until(id, |window| window.tree.get_element_by_id(id).is_some());
}

fn sign_in(window: &mut Window) {
  let services = window.launch.services().expect("services").clone();
  signed_in::sign_in(&services, window.data.runtime.handle(), &window.data.cloud);
  window.pass();
}

fn sign_out(window: &mut Window) {
  let services = window.launch.services().expect("services").clone();
  window
    .data
    .runtime
    .block_on(services.cloud.sign_out())
    .expect("signed out");
  window.pass();
}

#[test]
fn local_use_a_fresh_install_works_with_the_cloud_asked_nothing() {
  let mut window = fresh();
  window.until("the welcome", |window| {
    window.tree.get_element_by_id("welcome-introduction").is_some()
  });
  assert!(
    window.tree.get_element_by_id("tab-strip").is_some(),
    "the window is the normal one"
  );

  let services = window.launch.services().expect("services").clone();
  window.data.add_kontur(&services.workspace);
  window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::NewTask));
  shows(&mut window, "sidebar");
  shows(&mut window, "task-form-title-input");
  window.set("task-form-title-input", "Write the delivery docs");
  window.press("task-form-create");
  let created = Route::Project(
    project_id(),
    ProjectPage::Tasks(TaskList::new(TaskListTab::All).with_highlight("KON-1".parse().expect("a key"))),
  );
  window.until("the new task in Tasks", |window| {
    window.navigation().route_now() == created
  });
  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Settings(ProjectSettingsTab::General),
  ));
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  shows(&mut window, "sign-in-email");
  window.navigation().go(Route::AllProjects);
  shows(&mut window, "project-list");
  assert_eq!(window.data.cloud.requests(), 0, "no request reached the cloud");
}

#[test]
fn local_use_sharing_offers_the_sign_in_in_place() {
  let mut window = fresh();
  let services = window.launch.services().expect("services").clone();
  window.data.add_kontur(&services.workspace);
  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Settings(ProjectSettingsTab::Sharing),
  ));
  shows(&mut window, "sharing-sign-in");
  assert_eq!(
    window.text("sharing-share-hint"),
    "Sharing to a team needs an Orchester Cloud sign-in, or a code from the team server"
  );
  assert!(window.tree.get_element_by_id("sharing-share-open").is_none());
  assert!(
    window.tree.get_element_by_id("sign-in-email").is_none(),
    "it opens when asked"
  );

  window.press("sharing-sign-in");
  shows(&mut window, "sign-in-email");
  let route = window.navigation().route_now();
  window.set("sign-in-email", EMAIL);
  window.set("sign-in-password", signed_in::account_cloud::PASSWORD);
  window.press("sign-in-submit");
  shows(&mut window, "sharing-share-open");
  assert_eq!(
    window.navigation().route_now(),
    route,
    "no navigation: the row offers Share… again"
  );
  assert!(window.tree.get_element_by_id("sign-in-email").is_none());
}

#[test]
fn local_use_sharing_a_code_needs_no_sign_in() {
  let mut window = fresh();
  let services = window.launch.services().expect("services").clone();
  window.data.add_kontur(&services.workspace);
  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Settings(ProjectSettingsTab::Sharing),
  ));
  shows(&mut window, "sharing-use-code");
  window.press("sharing-use-code");
  assert_eq!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Code));
}

#[test]
fn local_use_tabs_follow_the_sign_in() {
  let mut window = fresh();
  let services = window.launch.services().expect("services").clone();
  window.data.add_kontur(&services.workspace);
  window.navigation().go(Route::Settings);
  window.navigation().open_tab();
  window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Overview(Default::default())));
  shows(&mut window, "sidebar");
  let before = window.navigation().tabs();

  sign_in(&mut window);
  assert_eq!(window.navigation().tabs(), before, "the first sign-in keeps what shows");

  sign_out(&mut window);
  shows(&mut window, "sign-in-notice");
  assert_eq!(
    window.navigation().route_now(),
    Route::Orchester(OrchesterView::default()),
    "signed out with no tabs of its own: Settings says why and offers the sign-in"
  );

  sign_in(&mut window);
  window.until("the account's tabs, as it left them", |window| {
    window.navigation().tabs() == before
  });
}

#[test]
fn local_use_a_project_that_waits_is_followed_by_nothing() {
  let mut window = fresh();
  sign_in(&mut window);
  let services = window.launch.services().expect("services").clone();
  let held = add_project(
    &services.workspace,
    (orchester_core::id::ProjectId::generate(), "LED", "Ledger"),
    &window.data.repo("repo-ledger"),
  );
  let id = held.project.id();
  window
    .navigation()
    .go(Route::Project(id, ProjectPage::Overview(Default::default())));
  shows(&mut window, "sidebar");
  let projects = window.link.projects().expect("the window's projects");
  assert!(projects.followed().contains(&id), "signed in, it is followed");

  sign_out(&mut window);
  shows(&mut window, "sign-in-notice");
  assert!(
    !projects.followed().contains(&id),
    "signed out, no reader and no notification tells of it"
  );
  assert!(
    !window
      .navigation()
      .tabs()
      .tabs()
      .iter()
      .any(|tab| format!("{tab:?}").contains(&id.to_string())),
    "and no tab shows it"
  );
}
