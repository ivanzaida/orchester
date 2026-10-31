//! The steps after a team is joined, in the window (`docs/design/startup.md` decisions 5–6; `Startup · redesign`
//! `qzzYw`): a stand-in team server joined with a code. Its shared projects are listed with the first picked (05
//! `v5qbWM`), none yet is the empty state with *Check again* (12 `NaEPI`), and choosing one leads to *Link a folder*
//! (06 `go2aM`), whose folder is checked as it changes: a match goes on (`go2aM`), another `origin` offers *Link
//! anyway* (13b `tDFeg`), not a repository is an error under the row with nothing to press (13 `sN8Pa`). There is no
//! *Clone it here*: the `git clone` command with *Copy* is in its place. Only a person links.
//!
//! `cargo test -p orchester-app --test shared_projects shared_projects_`

#[path = "../support/mod.rs"]
mod support;

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use lurq::app::events::with_synthetic_input;
use orchester_api::test_support::team_server::{CODE, TeamServer, USERNAME};
use orchester_app::routes::{JoinWay, Route};
use support::{Data, Window, stand_ins::StandIns};

const HINT: &str = "github.com/acme/kontur";
const FOLDER: &str = "join-team-link-folder";
const SUBMIT: &str = "join-team-link-submit";

/// A team named Acme sharing Kontur and Billing, Kontur with a repository hint.
fn acme() -> TeamServer {
  let server = TeamServer::named("Acme");
  server.share(&[("Kontur", "kontur", Some(HINT)), ("Billing", "billing", None)]);
  server.own("billing");
  server
}

/// The window with Kontur in the list, joined to `server` with its code as a person's press joins, at its projects.
fn joined(server: &TeamServer) -> (Window, StandIns) {
  let data = Data::new();
  let launch = data.launch();
  data.add_kontur(launch.workspace().expect("the session started"));
  let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
  window.navigation().go(Route::JoinTeam(JoinWay::Code));
  shows(&mut window, "join-team-code-address");
  window.set("join-team-code-address", &server.url());
  window.set("join-team-code-code", CODE);
  window.press("join-team-code-submit");
  shows(&mut window, "join-team-code-joined");
  window.press("join-team-code-open");
  (window, stand_ins)
}

fn shows(window: &mut Window, id: &str) {
  window.until(id, |window| window.tree.get_element_by_id(id).is_some());
}

fn has(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

/// Where the element `id` starts down the window, and how high it is.
fn top_and_height(window: &mut Window, id: &str) -> (f32, f32) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.y, bounds.height)
}

/// A press of `id`, which can't be pressed now, leaves the window where it is and `folder` as it is.
fn presses_nothing(window: &mut Window, id: &str, folder: &Path) {
  let route = window.navigation().route_now();
  window.press(id);
  window.pass();
  assert_eq!(window.navigation().route_now(), route);
  assert!(!folder.join(".orchester").exists(), "nothing was linked");
}

/// A folder under `window`'s data, with a git repository whose `origin` is `origin` when it has one.
fn folder(window: &Window, name: &str, repository: Option<&str>) -> PathBuf {
  let folder = window.data.dir.path().join(name);
  fs::create_dir_all(&folder).expect("the folder");
  if let Some(origin) = repository {
    git(&folder, &["init", "-b", "main"]);
    git(&folder, &["remote", "add", "origin", origin]);
  }
  folder
}

fn git(root: &Path, args: &[&str]) {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

/// *Link a folder* for Kontur, from the team's projects.
fn at_link(server: &TeamServer) -> (Window, StandIns) {
  let (mut window, stand_ins) = joined(server);
  shows(&mut window, "join-team-projects-continue");
  window.press("join-team-projects-continue");
  shows(&mut window, FOLDER);
  (window, stand_ins)
}

#[test]
fn shared_projects_lists_the_teams_projects_with_the_first_picked() {
  let server = acme();
  let (mut window, _) = joined(&server);
  shows(&mut window, "join-team-projects-kontur");
  assert_eq!(window.text("join_team-title"), "Pick a project from Acme");
  assert_eq!(window.text("join_team-crumbs"), "Start Join a team Acme");
  assert_eq!(
    window.text("join_team-steps"),
    "Join the team 2 Choose a project 3 Link a folder 4 Download tasks"
  );
  assert_eq!(window.text("join-team-projects-joined"), "Joined");
  assert_eq!(window.text("join-team-projects-kontur-title"), "Kontur");
  assert_eq!(window.text("join-team-projects-kontur-role"), "you: member");
  assert_eq!(window.text("join-team-projects-billing-role"), "you: owner");
  assert!(
    window
      .text("join-team-projects-kontur-sub")
      .ends_with(&format!("/kontur · remote {HINT}"))
  );
  assert_eq!(window.text("join-team-projects-continue"), "Continue with Kontur");

  window.press("join-team-projects-billing");
  window.until("Billing to be picked", |window| {
    window.text("join-team-projects-continue") == "Continue with Billing"
  });
  window.press("join-team-projects-kontur");
  window.until("Kontur to be picked", |window| {
    window.text("join-team-projects-continue") == "Continue with Kontur"
  });

  window.press("join-team-projects-continue");
  shows(&mut window, FOLDER);
  assert!(matches!(
    window.navigation().route_now(),
    Route::JoinTeam(JoinWay::Folder(target)) if target.slug() == "kontur"
  ));
  assert_eq!(window.text("join_team-title"), "Link Kontur to a folder");
  assert_eq!(window.text("join_team-crumbs"), "Start Join a team Acme Kontur");
  assert_eq!(
    window.text("join_team-steps"),
    "Join the team Choose a project 3 Link a folder 4 Download tasks"
  );
}

#[test]
fn shared_projects_none_yet_is_the_empty_state_and_check_again_reads_again() {
  let server = TeamServer::named("Acme");
  let (mut window, _) = joined(&server);
  shows(&mut window, "join-team-projects-none");
  assert_eq!(
    window.text("join-team-projects-none-title"),
    "No projects for you on Acme yet"
  );
  assert_eq!(window.text("join_team-crumbs"), "Start Join a team Acme");
  assert!(!has(&mut window, "join_team-lead"), "12 draws no lead");
  // Joined with a code, with nobody signed in to Orchester Cloud: the team knows the account it made there.
  let said = window.text("join-team-projects-none");
  assert!(said.contains(&format!("has invited {USERNAME}.")), "{said}");
  assert!(!said.contains('@'), "no Cloud email: {said}");

  server.share(&[("Kontur", "kontur", Some(HINT))]);
  window.press("join-team-projects-check-again");
  shows(&mut window, "join-team-projects-kontur");
  assert!(!has(&mut window, "join-team-projects-none"));
}

#[test]
fn shared_projects_back_to_start_opens_all_projects() {
  let server = TeamServer::named("Acme");
  let (mut window, _) = joined(&server);
  shows(&mut window, "join-team-projects-back-to-start");
  window.press("join-team-projects-back-to-start");
  assert_eq!(window.navigation().route_now(), Route::AllProjects);
}

#[test]
fn shared_projects_a_matching_folder_goes_on() {
  let server = acme();
  let (mut window, _) = at_link(&server);
  let nothing = window.data.dir.path().to_owned();
  presses_nothing(&mut window, SUBMIT, &nothing);

  let checkout = folder(&window, "kontur", Some("https://github.com/acme/kontur.git"));
  window.set(FOLDER, &checkout.display().to_string());
  shows(&mut window, "join-team-link-check");
  assert_eq!(
    window.text("join-team-link-check-text"),
    format!("Git repository · origin {HINT} matches the project")
  );
  assert_eq!(window.text(SUBMIT), "Link and download tasks");
}

#[test]
fn shared_projects_another_origin_offers_link_anyway() {
  let server = acme();
  let (mut window, _) = at_link(&server);
  let fork = folder(&window, "fork", Some("https://github.com/maria/kontur-fork.git"));
  window.set(FOLDER, &fork.display().to_string());
  shows(&mut window, "join-team-link-differs");
  assert_eq!(
    window.text("join-team-link-differs-title"),
    "origin is github.com/maria/kontur-fork"
  );
  assert_eq!(
    window.text("join-team-link-differs-body"),
    format!("Kontur expects {HINT}. Link anyway if this is your fork; runs push to its origin.")
  );
  assert_eq!(window.text(SUBMIT), "Link anyway");
  assert!(
    !has(&mut window, "join-team-link-error"),
    "it warns, it does not refuse"
  );
}

#[test]
fn shared_projects_not_a_repository_errors_under_the_row() {
  let server = acme();
  let (mut window, _) = at_link(&server);
  let plain = folder(&window, "plain", None);
  window.set(FOLDER, &plain.display().to_string());
  shows(&mut window, "join-team-link-error");
  assert_eq!(
    window.text("join-team-link-error"),
    "Not a git repository. Pick the folder you cloned, or clone it with the command below."
  );
  presses_nothing(&mut window, SUBMIT, &plain);

  let field = top_and_height(&mut window, &format!("{FOLDER}-box"));
  let choose = top_and_height(&mut window, "join-team-link-choose");
  let (error, _) = top_and_height(&mut window, "join-team-link-error");
  assert_eq!(field, choose, "Choose… lines up with the field's box");
  assert!(error >= field.0 + field.1, "the error is under the row");

  let other = folder(&window, "kontur", Some("https://github.com/acme/kontur.git"));
  window.set(FOLDER, &other.display().to_string());
  shows(&mut window, "join-team-link-check");
  assert!(!has(&mut window, "join-team-link-error"), "another folder clears it");
}

#[test]
fn shared_projects_a_relative_path_errors_under_the_row() {
  let server = acme();
  let (mut window, _) = at_link(&server);
  let checkout = folder(&window, "kontur", Some("https://github.com/acme/kontur.git"));
  window.set(FOLDER, "kontur");
  shows(&mut window, "join-team-link-error");
  assert_eq!(
    window.text("join-team-link-error"),
    "Give the folder's full path, from its drive or from /, or choose the folder."
  );
  presses_nothing(&mut window, SUBMIT, &checkout);

  // A pasted path keeps no spaces around it.
  window.set(FOLDER, &format!("  {}  ", checkout.display()));
  shows(&mut window, "join-team-link-check");
  assert!(!has(&mut window, "join-team-link-error"));
}

#[test]
fn shared_projects_choose_fills_the_field_from_the_picker() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  let checkout = folder(&window, "kontur", Some("git@github.com:acme/kontur.git"));
  stand_ins.choose(Some(checkout));
  window.press("join-team-link-choose");
  shows(&mut window, "join-team-link-check");
  assert_eq!(
    window.text("join-team-link-check-text"),
    format!("Git repository · origin {HINT} matches the project")
  );
}

#[test]
fn shared_projects_there_is_no_clone_only_the_command_with_copy() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  let command = format!("git clone https://{HINT}.git");
  assert_eq!(window.text("join-team-link-clone-command"), command);
  window.press("join-team-link-clone-command-copy");
  window.until("the command to be copied", |_| stand_ins.copied.lock().len() == 1);
  assert_eq!(stand_ins.copied.lock().as_slice(), [command]);
}

#[test]
fn shared_projects_a_project_without_a_hint_has_no_command() {
  let server = acme();
  let (mut window, _) = joined(&server);
  shows(&mut window, "join-team-projects-billing");
  window.press("join-team-projects-billing");
  window.until("Billing to be picked", |window| {
    window.text("join-team-projects-continue") == "Continue with Billing"
  });
  window.press("join-team-projects-continue");
  shows(&mut window, FOLDER);
  assert!(!has(&mut window, "join-team-link-clone"));
  let plain = folder(&window, "plain", None);
  window.set(FOLDER, &plain.display().to_string());
  shows(&mut window, "join-team-link-error");
  assert_eq!(
    window.text("join-team-link-error"),
    "Not a git repository. Pick the folder you cloned."
  );
}

#[test]
fn shared_projects_only_a_person_links() {
  let server = acme();
  let (mut window, _) = at_link(&server);
  let checkout = folder(&window, "kontur", Some("https://github.com/acme/kontur.git"));
  window.set(FOLDER, &checkout.display().to_string());
  shows(&mut window, "join-team-link-check");
  with_synthetic_input(|| window.press(SUBMIT));
  shows(&mut window, &format!("{SUBMIT}-person-only"));
  assert!(!checkout.join(".orchester").exists(), "nothing was linked");
}

#[test]
fn shared_projects_linking_opens_the_project() {
  let server = acme();
  let (mut window, _) = at_link(&server);
  let checkout = folder(&window, "kontur", Some("https://github.com/acme/kontur.git"));
  window.set(FOLDER, &checkout.display().to_string());
  shows(&mut window, "join-team-link-check");
  window.press(SUBMIT);
  window.until("the project to open", |window| {
    matches!(window.navigation().route_now(), Route::Project(..))
  });
  assert!(checkout.join(".orchester").exists(), "the folder was set up");
}
