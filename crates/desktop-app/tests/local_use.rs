//! Local use without an Orchester Cloud account (the person's decision, 2026-10-28): a fresh install opens the
//! workspace, adds a project, creates a task and opens its pages with the cloud asked nothing; what needs a sign-in
//! says so in place and offers it; the tabs follow a sign-in and a sign-out; a project that waits for its account tells
//! nothing meanwhile; and every page that confirms a first link says when it moves the project into the account, which
//! it does only once the server accepted. Running a task to its end needs a real runner: `mcp_domain`'s
//! `domain_signed_out_a_task_runs_to_its_end_and_merges` does that signed out.
//!
//! `cargo test -p orchester-app --test local_use local_use_`

mod support;

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
  test_support::team_server::{CODE, TasksAnswer, TeamServer, USERNAME},
};
use orchester_app::routes::{
  JoinTarget, JoinWay, OrchesterView, ProjectPage, ProjectSettingsTab, Route, TaskList, TaskListTab,
};
use orchester_core::id::ServerId;
use orchester_workspace::sharing::{CodeRequest, Trust, team_join::JoinOutcome};
use support::{
  Data, Window, add_project, project_id,
  signed_in::{self, account_cloud::EMAIL},
};

/// The address of the stand-in team server's project.
const SLUG: &str = "team";
/// The line that says a first link moves the project.
const MOVES: &str = "sharing-moves-to-account-text";
const MOVES_WORDS: &str = "Kontur moves into your account, so it shows only while you're signed in.";

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

/// x, y, width and height of `id`, once it is laid out.
fn rect(window: &mut Window, id: &str) -> (f32, f32, f32, f32) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.x, bounds.y, bounds.width, bounds.height)
}

#[test]
fn local_use_sharing_sign_in_sits_in_the_drawn_padding() {
  let mut window = fresh();
  let services = window.launch.services().expect("services").clone();
  window.data.add_kontur(&services.workspace);
  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Settings(ProjectSettingsTab::Sharing),
  ));
  shows(&mut window, "sharing-sign-in");
  window.press("sharing-sign-in");
  shows(&mut window, "sign-in-email");
  window.pass();
  let (_, row_y, _, row_height) = rect(&mut window, "sharing-share-row");
  let (under_x, under_y, under_width, under_height) = rect(&mut window, "sharing-sign-in-under");
  let (card_x, card_y, card_width, card_height) = rect(&mut window, "sign-in-card");
  let same = |a: f32, b: f32| (a - b).abs() < 0.5;
  assert!(same(under_y, row_y + row_height), "right under the row");
  assert!(
    same(card_x - under_x, 14.0) && same(card_y - under_y, 14.0),
    "14 at its top and start (`CLPfB` in `i5lddT`)"
  );
  assert!(
    same(under_x + under_width - (card_x + card_width), 14.0),
    "14 at its end"
  );
  assert!(
    same(under_y + under_height - (card_y + card_height), 16.0),
    "16 under it"
  );
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
fn local_use_signing_in_again_for_a_code_opens_the_sign_in_from_kept_tabs() {
  let mut window = fresh();
  sign_in(&mut window);
  window.navigation().go(Route::Settings);
  sign_out(&mut window);
  shows(&mut window, "sign-in-notice");
  window.navigation().go(Route::AllProjects);
  sign_in(&mut window);
  window.until("the account's tabs", |window| {
    window.navigation().route_now() == Route::Settings
  });

  let services = window.launch.services().expect("services").clone();
  window
    .data
    .runtime
    .block_on(services.cloud.sign_out_for_code(Some("Billing".into())))
    .expect("signed out for a code");
  window.until("the sign-in, not the page the signed-out tabs kept", |window| {
    window.navigation().route_now() == Route::Orchester(OrchesterView::default())
  });
  shows(&mut window, "sign-in-email");
}

#[test]
fn local_use_connect_a_server_offers_the_sign_in_in_place() {
  let mut window = fresh();
  let team = TeamServer::start();
  window.navigation().go(Route::Orchester(OrchesterView::connect()));
  shows(&mut window, "orchester-connect-address");
  window.set("orchester-connect-address", &team.url());
  window.press("orchester-connect-continue");
  shows(&mut window, "orchester-connect-sign-in");
  assert_eq!(
    window.text("orchester-connect-by-account-reason"),
    "Sign in to Orchester Cloud first."
  );
  assert!(
    window.tree.get_element_by_id("sign-in-email").is_none(),
    "it opens when asked"
  );

  window.press("orchester-connect-sign-in");
  shows(&mut window, "sign-in-email");
  let route = window.navigation().route_now();
  window.set("sign-in-email", EMAIL);
  window.set("sign-in-password", signed_in::account_cloud::PASSWORD);
  window.press("sign-in-submit");
  window.until("the sign-in made", |window| {
    window.tree.get_element_by_id("orchester-connect-sign-in").is_none()
  });
  assert_eq!(window.navigation().route_now(), route, "no navigation");
  assert!(window.tree.get_element_by_id("sign-in-email").is_none());
  assert!(
    window
      .tree
      .get_element_by_id("orchester-connect-by-account-reason")
      .is_none(),
    "the way can be chosen now"
  );
  window.press("orchester-connect-by-account");
  window.until("the Orchester account's way chosen", |window| {
    window.tree.get_element_by_id("orchester-connect-code").is_none()
  });
}

/// The stand-in team server with the project at [`SLUG`], joined with a code from `window`.
fn joined_team(window: &Window) -> (TeamServer, ServerId) {
  let team = TeamServer::start();
  team.share(&[("Team project", SLUG, None)]);
  let services = window.launch.services().expect("services");
  let address = ServerAddress::parse(&team.url()).expect("the team server's address");
  let request = CodeRequest {
    code: Credential::new(CODE.to_owned()),
    username: Some(USERNAME.to_owned()),
    display_name: None,
    proof: None,
  };
  let outcome = window
    .data
    .runtime
    .block_on(
      services
        .team_join
        .join_by_code(&address, Trust::System(PlainHttp::NotAllowed), request),
    )
    .expect("the code joins");
  let JoinOutcome::Joined(joined) = outcome else {
    panic!("not joined: {outcome:?}");
  };
  (team, joined.id)
}

fn join_route(server: ServerId) -> Route {
  let target = JoinTarget::new(server, SLUG).expect("a join target");
  Route::Project(project_id(), ProjectPage::Join(target))
}

#[test]
fn local_use_the_pages_that_confirm_a_link_say_the_project_moves_into_the_account() {
  let mut window = fresh();
  let services = window.launch.services().expect("services").clone();
  window.data.add_kontur(&services.workspace);
  sign_in(&mut window);
  let (team, server) = joined_team(&window);

  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Settings(ProjectSettingsTab::Sharing),
  ));
  shows(&mut window, "sharing-share-open");
  window.press("sharing-share-open");
  shows(&mut window, MOVES);
  assert_eq!(window.text(MOVES), MOVES_WORDS, "the Share page");

  window.navigation().go(join_route(server));
  shows(&mut window, "sharing-link-confirm");
  assert_eq!(
    window.text(MOVES),
    MOVES_WORDS,
    "the first link's page, as an invite opens it"
  );

  window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::NewTask));
  shows(&mut window, "task-form-title-input");
  window.set("task-form-title-input", "Write the delivery docs");
  window.press("task-form-create");
  window.until("the new task", |window| {
    matches!(
      window.navigation().route_now(),
      Route::Project(_, ProjectPage::Tasks(_))
    )
  });
  team.answer_tasks(TasksAnswer::Serve);
  window.navigation().go(join_route(server));
  shows(&mut window, "sharing-join");
  assert_eq!(
    window.text(MOVES),
    MOVES_WORDS,
    "the join review, as an invite opens it"
  );
}

#[test]
fn local_use_a_link_to_a_server_of_this_computer_moves_nothing_and_says_nothing() {
  let mut window = fresh();
  let services = window.launch.services().expect("services").clone();
  window.data.add_kontur(&services.workspace);
  let (_team, server) = joined_team(&window);

  window.navigation().go(join_route(server));
  shows(&mut window, "sharing-link-confirm");
  assert!(window.tree.get_element_by_id(MOVES).is_none());
  window.press("sharing-link-confirm");
  window.until("the link", |window| {
    let services = window.launch.services().expect("services");
    services
      .workspace
      .projects()
      .expect("the list")
      .first()
      .is_some_and(|project| {
        orchester_store::ProjectDb::open(&project.repo_root)
          .and_then(|db| db.sharing())
          .is_ok_and(|link| link.is_some())
      })
  });
  let listed = services.workspace.projects().expect("the list");
  assert_eq!(listed.len(), 1, "it still shows with nobody signed in");
  assert_eq!(listed[0].owner, None, "it stays this computer's");
}

#[test]
fn local_use_a_link_that_failed_says_so_and_moves_nothing() {
  let mut window = fresh();
  let services = window.launch.services().expect("services").clone();
  window.data.add_kontur(&services.workspace);
  sign_in(&mut window);
  let (team, server) = joined_team(&window);

  window.navigation().go(join_route(server));
  shows(&mut window, "sharing-link-confirm");
  team.refuse_links();
  window.press("sharing-link-confirm");
  shows(&mut window, "sharing-upload-failed");
  let listed = services.workspace.projects().expect("the list");
  assert_eq!(listed[0].owner, None, "it stays this computer's");
  assert_eq!(
    window.text(MOVES),
    MOVES_WORDS,
    "and the page still says what a link would do"
  );
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
