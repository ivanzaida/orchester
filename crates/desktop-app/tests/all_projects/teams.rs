//! All projects with the person's teams (09 `i5G19A`, 09b `dLOCJ`): *Join a team* and *Open shared project…* in the
//! header, where each project is, the team project with no checkout here offered above the list until *Not now*, and
//! without a team the line that says how to see one's projects. The team is a stand-in server on loopback, joined by
//! code as the start page joins it.

use std::{fs, path::Path};

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
  test_support::team_server::{CODE, TeamServer, USERNAME},
};
use orchester_app::{
  routes::{JoinTarget, OrchesterView, Route},
  startup::Launch,
};
use orchester_core::{
  id::{ProjectId, ServerId},
  project::ProjectKey,
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharingLink};
use orchester_workspace::sharing::{CodeRequest, Trust, team_join::JoinOutcome};

use super::{
  all_projects,
  support::{Data, Window, add_project, project_id},
};

const TEAM: &str = "Acme";
const OFFER: &str = "all-projects-offer";
const HINT: &str = "all-projects-team-hint";

fn place(project: ProjectId) -> String {
  format!("project-row-where-{project}")
}

/// Joins `server` by its code from the session of `launch`, as the start page does; the team's id in the list.
fn join(data: &Data, launch: &Launch, server: &TeamServer) -> ServerId {
  let services = launch.services().expect("the session started");
  let address = ServerAddress::parse(&server.url()).expect("the team's address");
  let request = CodeRequest {
    code: Credential::new(CODE.to_owned()),
    username: Some(USERNAME.to_owned()),
    display_name: None,
    proof: None,
  };
  let outcome = data.runtime.block_on(services.team_join.join_by_code(
    &address,
    Trust::System(PlainHttp::NotAllowed),
    request,
  ));
  match outcome {
    Ok(JoinOutcome::Joined(joined)) => joined.id,
    other => panic!("not joined: {other:?}"),
  }
}

/// The id on `server` of its project `name`.
fn server_project(data: &Data, launch: &Launch, server: ServerId, name: &str) -> ProjectId {
  let services = launch.services().expect("the session started");
  data
    .runtime
    .block_on(services.team_join.projects(server))
    .expect("the team's projects")
    .into_iter()
    .find(|listed| listed.project.name == name)
    .unwrap_or_else(|| panic!("{name} is shared"))
    .project
    .id
}

/// Links the project at `repo` to `project` on `server`, as linking a checkout does.
fn link(repo: &Path, (server, project): (ServerId, ProjectId)) {
  let mut db = ProjectDb::open(repo).expect("the project's data");
  let link = SharingLink {
    server,
    project,
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Member),
    joined_at: Timestamp::from_unix_millis(100),
    cursor: 1,
    last_synced_at: Timestamp::from_unix_millis(100),
    removed_at: None,
    address: None,
  };
  db.start_sharing(&link, &[], (&[], &Default::default()))
    .expect("linked");
}

#[test]
fn all_projects_header_leads_to_joining_a_team() {
  let mut window = Window::open();
  all_projects(&mut window);
  let order = [
    "all-projects-join-team",
    "all-projects-open-shared",
    "all-projects-open-folder",
    "all-projects-add",
  ];
  let starts: Vec<f32> = order
    .iter()
    .map(|id| {
      window
        .tree
        .get_element_by_id_mut(id)
        .and_then(|element| element.bounds())
        .unwrap_or_else(|| panic!("{id} is laid out"))
        .x
    })
    .collect();
  assert!(starts.is_sorted(), "{order:?} in this order: {starts:?}");
  assert_eq!(window.text("all-projects-join-team"), "Join a team");
  assert_eq!(window.text("all-projects-open-shared"), "Open shared project…");
  window.press("all-projects-join-team");
  assert_eq!(
    window.navigation().route_now(),
    Route::Orchester(OrchesterView::default())
  );
  all_projects(&mut window);
  window.until("the teams to be read", |window| window.texts(HINT).is_some());
  window.press("all-projects-open-shared");
  assert_eq!(
    window.navigation().route_now(),
    Route::Orchester(OrchesterView::default()),
    "with no team joined, a team is joined first"
  );
  window.navigation().go(Route::Usage(Box::default()));
  window.until("the Usage tab", |window| {
    window.tree.get_element_by_id("all-projects-top").is_some()
  });
  assert!(
    window.texts("all-projects-join-team").is_some() && window.texts("all-projects-open-shared").is_some(),
    "the Usage tab keeps the header's actions"
  );
}

#[test]
fn all_projects_says_where_each_project_is() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("the session started");
  let services = launch.services().expect("the session started");
  data.add_kontur(workspace);
  let acme = data
    .runtime
    .block_on(services.servers.add(
      &ServerAddress::parse("https://orchester.acme.dev").expect("address"),
      TEAM.to_owned(),
      PlainHttp::NotAllowed,
    ))
    .expect("the team server is in the list");
  let billing = add_project(
    workspace,
    (ProjectId::generate(), "BIL", "Billing"),
    &data.repo("repo-billing"),
  );
  link(&billing.repo_root, (acme, ProjectId::generate()));
  let gone = add_project(
    workspace,
    (ProjectId::generate(), "GON", "Gone"),
    &data.repo("repo-gone"),
  );
  fs::remove_dir_all(&gone.repo_root).expect("the folder is gone");
  let mut window = Window::of(data, launch);
  all_projects(&mut window);
  let headings = window.text("project-list-headings");
  assert_eq!(headings, "PROJECT WHERE STATUS ACTIVE RUNS NEEDS YOU LAST OPENED");
  let billing = place(billing.project.id());
  window.until("where Billing is", |window| {
    window.texts(&billing).is_some_and(|texts| texts != ["—"])
  });
  assert_eq!(window.text(&billing), "Shared · Acme");
  assert_eq!(window.text(&place(project_id())), "Local");
  assert_eq!(
    window.text(&place(gone.project.id())),
    "—",
    "a missing folder says nothing"
  );
}

#[test]
fn all_projects_offers_a_team_project_once_with_not_now() {
  let server = TeamServer::named(TEAM);
  server.share(&[
    ("Kontur", "kontur", None),
    ("Billing", "billing", None),
    ("Atlas", "atlas", None),
  ]);
  let data = Data::new();
  let launch = data.launch();
  let kontur = data.add_kontur(launch.workspace().expect("the session started"));
  let team = join(&data, &launch, &server);
  link(
    &kontur.repo_root,
    (team, server_project(&data, &launch, team, "Kontur")),
  );
  let mut window = Window::of(data, launch);
  all_projects(&mut window);
  window.until("Billing to be offered", |window| window.texts(OFFER).is_some());
  assert_eq!(
    window.text(OFFER),
    "Acme shares Billing with you. It isn't on this computer yet. Open Billing… Not now"
  );
  window.until("where Kontur is", |window| {
    window
      .texts(&place(project_id()))
      .is_some_and(|texts| texts == ["Shared · Acme"])
  });
  assert!(window.texts(HINT).is_none(), "a team is joined");
  window.press("all-projects-open-shared");
  assert_eq!(
    window.navigation().route_now(),
    Route::Orchester(OrchesterView::server(team)),
    "the one team's projects"
  );
  all_projects(&mut window);
  window.until("Billing to be offered again", |window| window.texts(OFFER).is_some());
  window.press("all-projects-offer-open");
  assert_eq!(
    window.navigation().route_now(),
    Route::Invite(JoinTarget::new(team, "billing").expect("a slug")),
    "a checkout of Billing is linked on its landing"
  );
  all_projects(&mut window);
  window.until("Billing to be offered again", |window| window.texts(OFFER).is_some());
  window.press("all-projects-offer-not-now");
  assert!(window.texts(OFFER).is_none(), "Not now puts it off at once");
  window.until("the next project to be offered", |window| window.texts(OFFER).is_some());
  assert!(
    window.text(OFFER).starts_with("Acme shares Atlas with you."),
    "once Billing is put off, the next one: {}",
    window.text(OFFER)
  );
  let Window { data, launch, .. } = window;
  let mut window = Window::of(data, launch);
  all_projects(&mut window);
  window.until("a new window's offer", |window| window.texts(OFFER).is_some());
  assert!(
    window.text(OFFER).starts_with("Acme shares Atlas with you."),
    "Billing stays put off in a new window: {}",
    window.text(OFFER)
  );
}

#[test]
fn all_projects_cuts_a_long_team_name_to_its_column() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("the session started");
  let services = launch.services().expect("the session started");
  data.add_kontur(workspace);
  let team = "Orchester Platform and Partner Integrations";
  let server = data
    .runtime
    .block_on(services.servers.add(
      &ServerAddress::parse("https://orchester.acme.dev").expect("address"),
      team.to_owned(),
      PlainHttp::NotAllowed,
    ))
    .expect("the team server is in the list");
  let billing = add_project(
    workspace,
    (ProjectId::generate(), "BIL", "Billing"),
    &data.repo("repo-billing"),
  );
  link(&billing.repo_root, (server, ProjectId::generate()));
  let mut window = Window::of(data, launch);
  all_projects(&mut window);
  let cell = place(billing.project.id());
  let badge = format!("{cell}-badge");
  window.until("where Billing is", |window| {
    window.tree.get_element_by_id(&badge).is_some()
  });
  let bounds = |window: &mut Window, id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (column, shown, words) = (
    bounds(&mut window, &cell),
    bounds(&mut window, &badge),
    bounds(&mut window, &format!("{badge}-label")),
  );
  let status = bounds(&mut window, &format!("project-row-status-{}", billing.project.id()));
  assert!(
    shown.x + shown.width <= column.x + column.width + 0.5,
    "the badge keeps to its column: {shown:?} in {column:?}"
  );
  assert!(
    words.x + words.width <= shown.x + shown.width + 0.5,
    "its words keep to the badge: {words:?} in {shown:?}"
  );
  assert!(
    words.x + words.width < status.x,
    "the words end before the status: {words:?}, {status:?}"
  );
}

#[test]
fn all_projects_offers_nothing_without_a_team() {
  let mut window = Window::open();
  all_projects(&mut window);
  window.until("the teams to be read", |window| window.texts(HINT).is_some());
  assert_eq!(
    window.text(HINT),
    "Working with a team? Join it to see the projects it shares with you."
  );
  assert_eq!(window.text(&place(project_id())), "Local");
  window.pass();
  assert!(window.texts(OFFER).is_none(), "no team, no offer");
}
