//! All projects and the welcome of a first launch: what the list says of each project, opening one, removing one
//! in place, the empty list, and *Open project folder…* with a stand-in picker. Projects live in repositories of
//! the test's own data folder.

mod support;

use std::{fs, path::Path};

use orchester_app::routes::{OverviewTab, ProjectPage, Route};
use orchester_core::id::ProjectId;
use support::{Data, Window, add_project, project_id};

const SCREEN: &str = "screen-all_projects";

fn all_projects(window: &mut Window) {
  window.navigation().go(Route::AllProjects);
  window.until("All projects to be read", |window| {
    window.tree.get_element_by_id("project-list").is_some()
      || window.tree.get_element_by_id("all-projects-empty").is_some()
  });
}

fn row(project: ProjectId) -> String {
  format!("project-row-{project}")
}

/// A project whose data was written by a newer Orchester.
fn newer(repo: &Path) {
  let connection =
    rusqlite::Connection::open(repo.join(".orchester").join("orchester.db")).expect("the project's data");
  connection
    .pragma_update(None, "user_version", 999)
    .expect("a newer schema version");
}

#[test]
fn all_projects_welcomes_a_first_launch() {
  let (mut window, _stand_ins) = Window::with_stand_ins(Data::new());
  window.until("the welcome", |window| {
    window.tree.get_element_by_id("welcome-introduction").is_some()
  });
  let texts = window.text(SCREEN);
  assert!(texts.contains("WELCOME TO ORCHESTER"), "{texts}");
  assert!(texts.contains("THREE STEPS TO YOUR FIRST TASK"), "{texts}");
  assert!(texts.contains("No projects yet"), "{texts}");
  assert!(window.tree.get_element_by_id("sidebar").is_none(), "no sidebar");
  window.press("welcome-create-project");
  assert_eq!(window.navigation().route_now(), Route::AddProject(None));
}

#[test]
fn all_projects_lists_projects_with_where_they_stand() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("the session started");
  let gone = add_project(
    workspace,
    (ProjectId::generate(), "GON", "Gone"),
    &data.repo("repo-gone"),
  );
  let old = add_project(
    workspace,
    (ProjectId::generate(), "LEG", "Legacy CRM"),
    &data.repo("repo-legacy"),
  );
  data.add_kontur(workspace);
  fs::remove_dir_all(&gone.repo_root).expect("the folder is gone");
  newer(&old.repo_root);
  let mut window = Window::of(data, launch);
  all_projects(&mut window);
  assert!(window.tree.get_element_by_id("sidebar").is_none(), "no sidebar");
  let headings = window.text("project-list-headings");
  assert_eq!(headings, "PROJECT STATUS ACTIVE RUNS NEEDS YOU LAST OPENED");
  window.until("the counts to be read", |window| {
    window.text(&row(project_id())).contains(" 0 0 Just now")
  });
  let kontur = window.text(&row(project_id()));
  assert!(kontur.starts_with("K Kontur"), "{kontur}");
  assert!(kontur.contains(" 0 0 Just now"), "the counts are read: {kontur}");
  let missing = window.text(&row(gone.project.id()));
  assert!(missing.contains("The folder is not there any more."), "{missing}");
  assert!(missing.contains("Folder missing — — "), "never a made-up 0: {missing}");
  let unreadable = window.text(&row(old.project.id()));
  assert!(
    unreadable.contains("The data was written by a newer version of Orchester. Can’t be opened"),
    "{unreadable}"
  );
  let note = window.text("all-projects-removal-note");
  assert!(
    note.starts_with("Removing a project from this list only makes Orchester forget it."),
    "{note}"
  );
  let usage = window.text("all-projects-tab-usage");
  assert_eq!(usage, "Usage", "Usage across projects is locked");
}

#[test]
fn all_projects_opens_a_project_on_its_overview() {
  let mut window = Window::open();
  all_projects(&mut window);
  window.press(&row(project_id()));
  assert_eq!(
    window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Overview(OverviewTab::default()))
  );
}

#[test]
fn all_projects_removes_a_project_in_place_and_keeps_its_data() {
  let mut window = Window::open();
  all_projects(&mut window);
  let remove = format!("project-remove-{}", project_id());
  window.press(&remove);
  let question = window.text("remove-project-question");
  assert!(question.starts_with("Remove Kontur from the list?"), "{question}");
  assert!(
    question.contains(".orchester stays on disk · tasks, runs and settings"),
    "{question}"
  );
  window.press("remove-project-cancel");
  assert!(
    window.texts("remove-project-question").is_none(),
    "Cancel keeps the project"
  );
  window.press(&remove);
  window.press("remove-project-confirm");
  window.until("the empty list", |window| {
    window.tree.get_element_by_id("all-projects-empty").is_some()
  });
  let empty = window.text("all-projects-empty");
  assert!(
    empty.starts_with("No projects in the list"),
    "every project removed is not a first launch: {empty}"
  );
  let workspace = window.launch.workspace().expect("the session").clone();
  assert!(workspace.projects().expect("the list").is_empty());
  let repo = window.data.dir.path().join("repo-kontur");
  assert!(
    repo.join(".orchester").join("orchester.db").is_file(),
    "the data stays on disk"
  );
}

#[test]
fn all_projects_open_folder_refuses_a_folder_without_a_project_and_opens_one_with() {
  let data = Data::new();
  let kontur = data.add_kontur(data.launch().workspace().expect("the session"));
  let plain = data.repo("repo-plain");
  let (mut window, stand_ins) = Window::with_stand_ins(data);
  all_projects(&mut window);
  stand_ins.choose(Some(plain.clone()));
  window.press("all-projects-open-folder");
  window.until("the refusal", |window| window.texts("all-projects-error").is_some());
  let error = window.text("all-projects-error");
  assert_eq!(
    error,
    format!(
      "Couldn’t open {} — it has no .orchester project data. Use Add project to set it up as a new project.",
      plain.display()
    )
  );
  stand_ins.choose(None);
  window.press("all-projects-open-folder");
  window.until("the line to go", |window| window.texts("all-projects-error").is_none());
  stand_ins.choose(Some(kontur.repo_root));
  window.press("all-projects-open-folder");
  window.until("Kontur to open", |window| {
    window.navigation().route_now() == Route::Project(project_id(), ProjectPage::Overview(OverviewTab::default()))
  });
}
