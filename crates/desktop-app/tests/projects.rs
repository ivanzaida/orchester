//! A route names a project by its id: it resolves to the open project, to no project in the list, or to
//! a project whose folder is gone; a session lands on the last used project, or on All projects at a
//! first launch. The window names the project in its tabs and its switcher, starts its runner, and
//! shows why a project cannot be opened in place of its screen.

mod support;

use std::fs;

use lurq::app::events::MouseButton;
use orchester_app::{
  project::{Landing, ProjectState, Unavailable, landing, resolve},
  routes::{OverviewTab, ProjectPage, Route},
};
use orchester_core::id::ProjectId;
use orchester_workspace::runner::state::{RunnerFailure, RunnerState};
use support::{Data, PROJECT, Window, add_project, project_id};

fn overview(project: ProjectId) -> Route {
  Route::Project(project, ProjectPage::Overview(OverviewTab::default()))
}

#[test]
fn project_ref_resolves_to_the_open_project() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("the session started");
  let added = data.add_kontur(workspace);
  let Ok(ProjectState::Open(open)) = resolve(workspace, project_id()) else {
    panic!("Kontur opens");
  };
  assert_eq!(open, added);
  assert_eq!(open.project.name().as_str(), "Kontur");
}

#[test]
fn project_ref_unknown_names_nothing() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("the session started");
  data.add_kontur(workspace);
  assert_eq!(
    resolve(workspace, ProjectId::generate()).expect("the list reads"),
    ProjectState::Unavailable(Unavailable::Unknown)
  );
}

#[test]
fn project_ref_folder_missing_keeps_its_name_and_folder() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("the session started");
  let repo = data.repo("repo-moved");
  let id = ProjectId::generate();
  add_project(workspace, (id, "MOV", "Moved"), &repo);
  fs::remove_dir_all(&repo).expect("the folder goes");
  let Ok(ProjectState::Unavailable(Unavailable::FolderMissing { name, repo_root })) = resolve(workspace, id) else {
    panic!("a project whose folder is gone");
  };
  assert_eq!((name.as_str(), repo_root), ("Moved", repo));
}

#[test]
fn project_first_launch_lands_on_all_projects() {
  let data = Data::new();
  let launch = data.launch();
  assert_eq!(launch.landing(), Some(&Landing::all_projects()));
}

#[test]
fn project_landing_opens_the_last_used_project() {
  let data = Data::new();
  let first = data.launch();
  let added = data.add_kontur(first.workspace().expect("the session started"));
  first.end();
  let next = data.launch();
  let landed = next.landing().expect("the session started");
  assert_eq!(landed.route, overview(project_id()));
  assert_eq!(landed.project, Some((project_id(), ProjectState::Open(added))));
  assert_eq!(
    landing(next.workspace().expect("the session started")).expect("the list reads"),
    *landed
  );
}

#[test]
fn project_landing_on_a_missing_folder_says_so() {
  let data = Data::new();
  let first = data.launch();
  data.add_kontur(first.workspace().expect("the session started"));
  first.end();
  fs::remove_dir_all(data.repo("repo-kontur")).expect("the folder goes");
  let next = data.launch();
  let landed = next.landing().expect("the session started");
  assert_eq!(landed.route, overview(project_id()));
  assert!(matches!(
    landed.project,
    Some((_, ProjectState::Unavailable(Unavailable::FolderMissing { .. })))
  ));
}

#[test]
fn project_window_names_the_project_and_starts_its_runner() {
  let data = Data::new();
  let first = data.launch();
  data.add_kontur(first.workspace().expect("the session started"));
  first.end();
  let launch = data.launch();
  let mut window = Window::of(data, launch);
  window.until("the runner to give up", |window| {
    matches!(runner(window), Some(RunnerState::Failed { .. }))
  });
  assert_eq!(window.texts("tab-1"), Some(vec!["Kontur · Overview".to_owned()]));
  assert!(window.texts("screen-overview").is_some());
  let repo = window.data.repo("repo-kontur").display().to_string();
  assert_eq!(
    window.texts("project-switcher"),
    Some(vec!["K".to_owned(), "Kontur".to_owned(), repo])
  );
  assert!(
    matches!(
      runner(&mut window),
      Some(RunnerState::Failed {
        cause: RunnerFailure::MissingExecutable { .. },
        ..
      })
    ),
    "the runner was started; the tests' runner program does not exist"
  );
  window.navigation().open_tab();
  window.pass();
  assert!(window.texts("sidebar").is_none(), "All projects has no sidebar");
  window.navigation().go(Route::Settings);
  window.pass();
  assert_eq!(
    window.texts("project-switcher").map(|texts| texts[0].clone()),
    Some("No project open".to_owned())
  );
}

/// The state of Kontur's runner, once its feed has one.
fn runner(window: &mut Window) -> Option<RunnerState> {
  let feed = window.projects().feed_now(project_id())?;
  Some(feed.status.get_untracked()?.state().clone())
}

#[test]
fn project_unknown_shows_no_such_project() {
  let mut window = Window::open();
  window.navigation().go(overview(ProjectId::generate()));
  window.until("the project to resolve", |window| {
    window.texts("screen-project-unknown").is_some()
  });
  assert_eq!(
    window.texts("screen-project-unknown").map(|texts| texts[0].clone()),
    Some("No such project".to_owned())
  );
  assert_eq!(
    window.texts("tab-1"),
    Some(vec!["Overview".to_owned()]),
    "no name to show"
  );
}

#[test]
fn project_folder_missing_shows_its_path() {
  let mut window = Window::open();
  let repo = window.data.repo("repo-kontur");
  fs::remove_dir_all(&repo).expect("the folder goes");
  window.navigation().go(overview(PROJECT.parse().expect("a project id")));
  window.until("the project to resolve", |window| {
    window.texts("screen-project-folder-missing").is_some()
  });
  let texts = window
    .texts("screen-project-folder-missing")
    .expect("the folder-missing screen shows");
  assert_eq!(texts[0], "The folder of Kontur is missing");
  assert!(texts.contains(&repo.display().to_string()), "{texts:?}");
  assert_eq!(window.texts("tab-1"), Some(vec!["Kontur · Overview".to_owned()]));
}

/// Clicks the middle of the element `id`.
fn click(window: &mut Window, id: &str) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
}

#[test]
fn project_folder_missing_opens_once_it_is_back_and_try_again_is_pressed() {
  let mut window = Window::open();
  let repo = window.data.repo("repo-kontur");
  let away = window.data.dir.path().join("repo-away");
  fs::rename(&repo, &away).expect("the folder goes");
  window.navigation().go(overview(project_id()));
  window.until("the project to resolve", |window| {
    window.texts("screen-project-folder-missing").is_some()
  });
  fs::rename(&away, &repo).expect("the folder comes back");
  click(&mut window, "screen-project-folder-missing-try-again");
  window.until("the project to open", |window| {
    window.texts("screen-overview").is_some()
  });
}

#[test]
fn project_unavailable_leads_to_all_projects() {
  let mut window = Window::open();
  window.navigation().go(overview(ProjectId::generate()));
  window.until("the project to resolve", |window| {
    window.texts("screen-project-unknown").is_some()
  });
  click(&mut window, "screen-project-unknown-all-projects");
  assert_eq!(window.navigation().route_now(), Route::AllProjects);
}
