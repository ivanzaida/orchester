//! Whose each project is, on All projects (the person's decisions, 2026-10-30; `LinZt`, `ggjyN`, `xx4ST`, `NpTDN`,
//! `qxq4t`, `MvIpm`): the *Where* column says it, a row under the pointer offers the move, which is asked in place and
//! only a person's answer makes, a shared project stays where it is, and while nobody is signed in a line says what
//! waits for a sign-in.

use lurq::app::events::with_synthetic_input;
use orchester_core::{
  id::{ProjectId, ServerId},
  project::ProjectKey,
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharingLink};

use super::{all_projects, row};
use crate::support::{Data, Window, add_project, project_id, signed_in};

/// A window signed in, on the account's projects.
fn signed_in_window(projects: &[(ProjectId, &str, &str, &str)]) -> Window {
  let data = Data::new();
  let launch = data.launch_signed_in();
  let workspace = launch.workspace().expect("the session started");
  for (id, key, name, folder) in projects {
    add_project(workspace, (*id, key, name), &data.repo(folder));
  }
  Window::of(data, launch)
}

fn add(window: &Window, id: ProjectId, (key, name): (&str, &str), folder: &str) {
  let workspace = &window.launch.services().expect("services").workspace;
  add_project(workspace, (id, key, name), &window.data.repo(folder));
}

fn place(window: &mut Window, project: ProjectId) -> String {
  let id = format!("project-row-where-{project}");
  window.until("where it is", |window| {
    window.tree.get_element_by_id(&id).is_some() && !window.text(&id).contains('—')
  });
  window.text(&id)
}

/// Rests the pointer on `project`'s row.
fn hover(window: &mut Window, project: ProjectId) {
  let bounds = window
    .tree
    .get_element_by_id_mut(&row(project))
    .and_then(|element| element.bounds())
    .expect("the row is laid out");
  window
    .tree
    .mouse_move(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.pass();
}

#[test]
fn all_projects_where_says_whose_a_project_is_and_a_row_offers_the_move() {
  let data = Data::new();
  let launch = data.launch();
  data.add_kontur(launch.workspace().expect("the session started"));
  let mut window = Window::of(data, launch);
  all_projects(&mut window);
  assert_eq!(place(&mut window, project_id()), "This computer");
  hover(&mut window, project_id());
  assert!(
    window
      .tree
      .get_element_by_id(&format!("project-move-{}", project_id()))
      .is_none(),
    "nobody signed in: no account to move it to"
  );

  let services = window.launch.services().expect("services").clone();
  signed_in::sign_in(&services, window.data.runtime.handle(), &window.data.cloud);
  let ledger = ProjectId::generate();
  add(&window, ledger, ("LED", "Ledger"), "repo-ledger");
  all_projects(&mut window);
  let move_link = format!("project-move-{}", project_id());
  window.until("the signed-in list", |window| {
    window.tree.get_element_by_id(&row(ledger)).is_some()
  });
  assert_eq!(
    place(&mut window, project_id()),
    "This computer",
    "no sign-in claims it"
  );
  assert_eq!(place(&mut window, ledger), "Your account");

  hover(&mut window, project_id());
  window.until("the offer", |window| {
    window.tree.get_element_by_id(&move_link).is_some()
  });
  assert_eq!(window.text(&move_link), "Move to account");
  assert!(
    window
      .tree
      .get_element_by_id(&format!("project-remove-{}", project_id()))
      .is_some(),
    "the × stays at the end of the offer"
  );
  hover(&mut window, ledger);
  let back = format!("project-move-{ledger}");
  window.until("the other offer", |window| {
    window.tree.get_element_by_id(&back).is_some()
  });
  assert_eq!(window.text(&back), "Move to this computer");
  assert!(
    window.tree.get_element_by_id(&move_link).is_none(),
    "only the row under the pointer offers it"
  );
}

#[test]
fn all_projects_a_move_is_asked_in_place_and_is_a_persons() {
  let ledger = ProjectId::generate();
  let mut window = signed_in_window(&[(ledger, "LED", "Ledger", "repo-ledger")]);
  all_projects(&mut window);
  assert_eq!(place(&mut window, ledger), "Your account");
  hover(&mut window, ledger);
  let link = format!("project-move-{ledger}");
  window.until("the offer", |window| window.tree.get_element_by_id(&link).is_some());
  window.press(&link);
  window.until("the question", |window| {
    window.tree.get_element_by_id("move-project-question").is_some()
  });
  assert_eq!(window.text("move-project-title"), "Move Ledger to this computer?");
  with_synthetic_input(|| window.press("move-project-confirm"));
  window.until("who answers", |window| {
    window
      .tree
      .get_element_by_id("move-project-confirm-person-only")
      .is_some()
  });
  assert_eq!(place(&mut window, ledger), "Your account", "automation moves nothing");

  window.press("move-project-keep");
  window.until("the question gone", |window| {
    window.tree.get_element_by_id("move-project-question").is_none()
  });
  hover(&mut window, ledger);
  window.until("the offer", |window| window.tree.get_element_by_id(&link).is_some());
  window.press(&link);
  window.until("the question", |window| {
    window.tree.get_element_by_id("move-project-confirm").is_some()
  });
  window.press("move-project-confirm");
  window.until("the move", |window| {
    window.text(&format!("project-row-where-{ledger}")) == "This computer"
  });
}

#[test]
fn all_projects_a_shared_project_stays_where_it_is() {
  let data = Data::new();
  let launch = data.launch_signed_in();
  let open = data.add_kontur(launch.workspace().expect("the session started"));
  let mut db = ProjectDb::open(&open.repo_root).expect("Kontur's data");
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
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
  let mut window = Window::of(data, launch);
  all_projects(&mut window);
  assert_eq!(place(&mut window, project_id()), "Shared");
  hover(&mut window, project_id());
  let note = format!("project-move-shared-{}", project_id());
  window.until("why not", |window| window.tree.get_element_by_id(&note).is_some());
  assert_eq!(
    window.text(&note),
    "A shared project stays where it is. Make it local first."
  );
  assert!(
    window
      .tree
      .get_element_by_id(&format!("project-move-{}", project_id()))
      .is_none()
  );
}

/// Back on All projects after a sign-in ended: the window first follows it to Settings, where the sign-in is.
fn back_on_all_projects(window: &mut Window) {
  window.until("the window to follow the sign-in", |window| {
    window.tree.get_element_by_id("sign-in-email").is_some()
  });
  all_projects(window);
}

#[test]
fn all_projects_says_what_waits_for_a_sign_in() {
  let (ledger, site) = (ProjectId::generate(), ProjectId::generate());
  let mut window = signed_in_window(&[
    (ledger, "LED", "Ledger", "repo-ledger"),
    (site, "SIT", "Site", "repo-site"),
  ]);
  let services = window.launch.services().expect("services").clone();
  window
    .data
    .runtime
    .block_on(services.cloud.sign_out())
    .expect("signed out");
  back_on_all_projects(&mut window);
  window.until("what waits", |window| {
    window.tree.get_element_by_id("all-projects-waiting-text").is_some()
  });
  assert_eq!(
    window.text("all-projects-waiting-text"),
    "2 projects wait for your Orchester Cloud sign-in. Their runners keep working."
  );
  assert!(
    window.tree.get_element_by_id(&row(ledger)).is_none(),
    "they are not listed"
  );
  assert_eq!(
    window.text("all-projects-nothing-here"),
    "No projects on this computer Add a project to work here without an account, or sign in to get to the ones that \
     wait."
  );
  window.press("all-projects-waiting-sign-in");
  window.until("the sign-in", |window| {
    window.tree.get_element_by_id("sign-in-email").is_some()
  });
}

#[test]
fn all_projects_says_why_a_sign_in_ended() {
  let mut window = signed_in_window(&[(ProjectId::generate(), "LED", "Ledger", "repo-ledger")]);
  window.data.cloud.end_token(Some("revoked"), 1_792_147_957_042);
  let services = window.launch.services().expect("services").clone();
  window
    .data
    .runtime
    .block_on(services.cloud.check_sign_in())
    .expect_err("the cloud ended the token");
  back_on_all_projects(&mut window);
  window.until("what waits", |window| {
    window.tree.get_element_by_id("all-projects-waiting-text").is_some()
  });
  let words = window.text("all-projects-waiting-text");
  assert!(words.starts_with("Your sign-in ended at "), "{words}");
  assert!(
    words.ends_with(", from another device or the cloud's account page. 1 project waits until you sign in again."),
    "{words}"
  );
  assert_eq!(window.text("all-projects-waiting-sign-in"), "Sign in again");
}
