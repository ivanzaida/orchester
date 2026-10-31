//! All projects and the first start: what the list says of each project, opening one, removing one in place, the
//! empty list, *Open project folder…* with a stand-in picker, the *Usage* tab, the three ways in of a first start
//! ([`first_start`]), the person's teams ([`teams`]) and that an agent CLI is not found ([`agent_cli`]). Projects live
//! in repositories of the test's own data folder.
//!
//! `cargo test -p orchester-app --test all_projects all_projects`

#[path = "all_projects/agent_cli.rs"]
mod agent_cli;
#[path = "all_projects/first_start.rs"]
mod first_start;
mod support;
#[path = "all_projects/teams.rs"]
mod teams;

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
  assert_eq!(headings, "PROJECT WHERE STATUS ACTIVE RUNS NEEDS YOU LAST OPENED");
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
    unreadable.contains("The data was written by a newer version of Orchester. — Can’t be opened"),
    "where it is cannot be read either: {unreadable}"
  );
  let note = window.text("all-projects-removal-note");
  assert!(
    note.starts_with("Removing a project from this list only makes Orchester forget it."),
    "{note}"
  );
  usage_tab(&mut window);
}

/// Without invites the notice is not in the page at all, so the list is the page's 20 under the tabs (`S5HDKf`),
/// not one more gap lower.
#[test]
fn all_projects_puts_the_list_20_under_its_tabs_without_invites() {
  let data = Data::new();
  let launch = data.launch();
  data.add_kontur(launch.workspace().expect("the session started"));
  let mut window = Window::of(data, launch);
  all_projects(&mut window);
  let mut bounds = |id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (tabs, list) = (bounds("all-projects-tabs"), bounds("project-list"));
  let gap = list.y - (tabs.y + tabs.height);
  assert!((gap - 20.0).abs() < 0.5, "the list is {gap} under the tabs");
  assert!(
    window.tree.get_element_by_id("all-projects-invites").is_none(),
    "no notice"
  );
}

/// The list as 09 `i5G19A` draws it, hairlines without an outline: 56 high rows, *Where* 450 and *Status* 622 into the
/// list; and the active tab at All projects says so, without a close button while it is the only tab (`jMVvA`).
#[test]
fn all_projects_lays_out_its_list_and_tab_as_frame_09() {
  let mut window = Window::open();
  all_projects(&mut window);
  let project = project_id();
  let mut bounds = |id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (list, shown) = (bounds("project-list"), bounds(&row(project)));
  let (place, status) = (
    bounds(&format!("project-row-where-{project}")),
    bounds(&format!("project-row-status-{project}")),
  );
  assert!((shown.height - 56.0).abs() < 0.5, "a row is {} high", shown.height);
  assert!(
    (place.x - list.x - 450.0).abs() < 0.5,
    "Where is {} in",
    place.x - list.x
  );
  assert!(
    (status.x - list.x - 622.0).abs() < 0.5,
    "Status is {} in",
    status.x - list.x
  );
  let tab = window.navigation().tabs_now().active().id();
  assert_eq!(window.text(&format!("tab-{tab}")), "All projects");
  assert!(
    window.tree.get_element_by_id(&format!("tab-{tab}-close")).is_none(),
    "the only tab at All projects cannot be closed"
  );
}

/// The *Usage* tab: every project's usage across the whole content panel, its top pinned, and back.
fn usage_tab(window: &mut Window) {
  window.press("all-projects-tab-usage");
  assert_eq!(window.navigation().route_now(), Route::Usage(Box::default()));
  window.until("every project's usage", |window| {
    window.tree.get_element_by_id("usage-summary-runs-value").is_some()
      && window.text("usage-readback").contains(" project")
  });
  let readback = window.text("usage-readback");
  assert!(readback.contains("· 1 project ·"), "Kontur is read: {readback}");
  assert!(
    readback.contains("Gone") && readback.contains("Legacy CRM") && readback.ends_with("could not be read"),
    "the projects that could not be read are named: {readback}"
  );
  assert!(
    window.tree.get_element_by_id("usage-filter-project").is_some(),
    "a Project filter"
  );
  // The whole content panel, 40 from its sides, with the heading, tabs and toolbar pinned over the scroll.
  let bounds = |window: &mut Window, id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (screen, filters) = (bounds(window, SCREEN), bounds(window, "usage-filters"));
  assert!(
    (filters.x - screen.x - 40.0).abs() < 1.0 && (screen.width - filters.width - 80.0).abs() < 1.0,
    "the filters span the panel less 40 at each side: {screen:?} {filters:?}"
  );
  assert!(
    window.tree.get_element_by_id("all-projects-scroll").is_none(),
    "not the Projects tab's column"
  );
  let (top, scroll) = (bounds(window, "all-projects-top"), bounds(window, "usage-scroll"));
  assert!(
    top.y + top.height <= scroll.y,
    "the heading and tabs sit over the scroll, pinned"
  );
  window.press("all-projects-tab-projects");
  assert_eq!(window.navigation().route_now(), Route::AllProjects);
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
  assert!(
    window.tree.get_element_by_id(&remove).is_none(),
    "a row at rest shows only its chevron"
  );
  window.hover(&row(project_id()));
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
  window.hover(&row(project_id()));
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
