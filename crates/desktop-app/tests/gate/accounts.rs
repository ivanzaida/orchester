//! Another Orchester Cloud account on the same computer (the person's decision, 2026-10-16: projects and team servers
//! per account): it gets tabs of its own and sees none of the first account's projects or servers, in Settings ›
//! Orchester or the status bar; the first account finds its tabs, projects and servers again when it signs back in.

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
};
use orchester_app::{
  project::ProjectState,
  routes::{OrchesterView, OverviewTab, ProjectPage, Route},
};
use orchester_workspace::sharing::{CodeRequest, Trust};

use super::{gone, has, server_support::TestServer, shows, signed_in, support::Window};
use crate::support::{
  project_id,
  signed_in::account_cloud::{EMAIL, OTHER_EMAIL, PASSWORD},
};

/// Signs the session out, then in as `email`.
fn switch_to(window: &mut Window, email: &str) {
  let cloud = window.launch.services().expect("services").cloud.clone();
  window
    .data
    .runtime
    .block_on(async {
      cloud.sign_out().await?;
      cloud.sign_in(email.into(), Credential::new(PASSWORD.into())).await
    })
    .expect("signed in");
  window.until("the window behind the gate", |window| has(window, "content"));
}

#[test]
fn gate_another_account_gets_its_own_tabs_and_none_of_the_projects() {
  let mut window = signed_in();
  let kontur = project_id();
  let overview = Route::Project(kontur, ProjectPage::Overview(OverviewTab::default()));
  window.navigation().go(overview.clone());
  window.navigation().open_tab();
  window.navigation().go(overview.clone());
  window.until("Kontur open", |window| {
    matches!(window.projects().state(kontur), Some(ProjectState::Open(_)))
  });
  let before = window.navigation().tabs();

  switch_to(&mut window, OTHER_EMAIL);
  let tabs = window.navigation().tabs();
  assert_eq!(tabs.tabs().len(), 1, "one tab of its own");
  assert_eq!(tabs.active().route(), &Route::AllProjects);
  window.until("All projects", |window| has(window, "screen-all_projects"));
  assert!(
    !has(&mut window, &format!("project-row-{kontur}")),
    "All projects lists none of the first account's"
  );
  window.navigation().go(overview);
  window.until("Kontur resolved", |window| window.projects().state(kontur).is_some());
  assert!(
    matches!(window.projects().state(kontur), Some(ProjectState::Unavailable(_))),
    "its route shows nothing of it"
  );

  switch_to(&mut window, EMAIL);
  assert_eq!(window.navigation().tabs(), before, "the first account's tabs again");
  window.until("Kontur open again", |window| {
    matches!(window.projects().state(kontur), Some(ProjectState::Open(_)))
  });
}

#[test]
fn gate_another_account_sees_none_of_the_first_accounts_servers() {
  let mut window = signed_in();
  let server = TestServer::start();
  let services = window.launch.services().expect("services").clone();
  let request = CodeRequest {
    code: server.code_for("bohdan", true),
    username: Some("bohdan".into()),
    display_name: Some("Bohdan Koval".into()),
    // The setup code, which needs no two-factor identity.
    proof: None,
  };
  let kontur = window
    .data
    .runtime
    .block_on(services.servers.join_with_code(
      &ServerAddress::parse(&server.url()).expect("address"),
      Trust::System(PlainHttp::NotAllowed),
      request,
    ))
    .expect("Bohdan joins the team server");
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  let row = format!("orchester-row-{kontur}");
  shows(&mut window, &row);
  shows(&mut window, "status-server");

  switch_to(&mut window, OTHER_EMAIL);
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  shows(&mut window, "orchester-no-servers");
  assert!(
    !has(&mut window, &row),
    "Settings › Orchester lists none of the first account's"
  );
  gone(&mut window, "status-server");

  switch_to(&mut window, EMAIL);
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  shows(&mut window, &row);
  shows(&mut window, "status-server");
}
