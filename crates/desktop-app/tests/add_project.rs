//! Add project: the folder chosen with a stand-in picker, the GitHub block for folders that need no `gh` (not a
//! repository, a repository without a remote, a remote that is not on GitHub), the name's error, creating the
//! project and a folder that holds one already, the folder the route names, and Back. Repositories are made with
//! git under this worktree's `.tmp/`; no `gh` is asked.

mod support;

use std::{
  path::{Path, PathBuf},
  process::Command,
};

use orchester_app::routes::{OverviewTab, ProjectPage, Route, RouteFolder};
use support::{Data, Window, stand_ins::StandIns};
use tempfile::TempDir;

/// Folders of the test's own under the worktree's `.tmp/`.
fn scratch() -> TempDir {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/all-projects/tests");
  std::fs::create_dir_all(&root).expect("a scratch folder");
  tempfile::tempdir_in(root).expect("a folder of the test's own")
}

fn git(root: &Path, args: &[&str]) {
  let output = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .output()
    .expect("git runs");
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

/// A git repository `name` in `scratch` without a remote.
fn repository(scratch: &TempDir, name: &str) -> PathBuf {
  let repo = scratch.path().join(name);
  std::fs::create_dir_all(&repo).expect("a folder");
  git(&repo, &["init", "--quiet"]);
  repo
}

/// The window on Add project, opened from All projects.
fn add_project() -> (Window, StandIns) {
  let (mut window, stand_ins) = Window::with_stand_ins(Data::new());
  window.navigation().go(Route::AddProject(None));
  window.until("Add project", |window| {
    window.tree.get_element_by_id("screen-add_project").is_some()
  });
  (window, stand_ins)
}

/// Chooses `folder` in the stand-in picker and waits for its detection.
fn choose(window: &mut Window, stand_ins: &StandIns, folder: &Path) {
  stand_ins.choose(Some(folder.to_path_buf()));
  window.press("choose-folder");
  window.until("the folder's detection", |window| {
    window
      .texts("github-detection-status")
      .is_some_and(|status| status != ["Checking…"])
  });
}

#[test]
fn add_project_waits_for_a_folder() {
  let (mut window, _stand_ins) = add_project();
  assert!(window.tree.get_element_by_id("sidebar").is_none(), "no sidebar");
  assert_eq!(window.text("add-project-crumbs"), "All projects New project");
  let rail = window.text("add-project-rail");
  assert!(
    rail.starts_with("PROJECT SETUP 1. Project & repository 2. Runner & agents Not available in this version yet"),
    "{rail}"
  );
  assert!(rail.ends_with("Back Leaving now discards what you entered."), "{rail}");
  assert_eq!(window.text("project-folder-value"), "No folder chosen");
  assert!(
    window
      .text("github-from-folder")
      .starts_with("GitHub is detected from the folder")
  );
  assert_eq!(
    window.text("add-project-status"),
    "Choose the project folder to continue."
  );
  window.press("create-project");
  assert_eq!(
    window.navigation().route_now(),
    Route::AddProject(None),
    "Create waits for a folder"
  );
}

#[test]
fn add_project_refuses_a_folder_that_is_not_a_repository() {
  let scratch = scratch();
  let plain = scratch.path().join("plain");
  std::fs::create_dir_all(&plain).expect("a folder");
  let (mut window, stand_ins) = add_project();
  choose(&mut window, &stand_ins, &plain);
  assert_eq!(
    window.text("github-detection-heading"),
    "This folder is not a git repository"
  );
  assert_eq!(window.text("github-detection-status"), "Not a repository");
  let block = window.text("github-detection");
  assert!(block.contains("Run this in a terminal, then check again."), "{block}");
  assert!(!block.contains("Optional"), "a repository is not optional: {block}");
  assert!(
    window.texts("git-init-command-box").is_some(),
    "the command to run: {block}"
  );
  assert_eq!(
    window.text("add-project-status"),
    "The project folder must be a git repository to continue."
  );
  window.set("project-name", "Kontur");
  window.press("create-project");
  let workspace = window.launch.workspace().expect("the session").clone();
  assert!(
    workspace.projects().expect("the list").is_empty(),
    "Create waits for a repository"
  );
}

#[test]
fn add_project_creates_a_local_project_and_opens_it() {
  let scratch = scratch();
  let repo = repository(&scratch, "kontur");
  let (mut window, stand_ins) = add_project();
  choose(&mut window, &stand_ins, &repo);
  assert_eq!(window.text("github-detection-heading"), "No GitHub remote");
  assert_eq!(window.text("github-detection-status"), "Local only");
  assert_eq!(
    window.text("add-project-status"),
    "Creates the project in the repository’s .orchester folder and opens it."
  );
  window.press("create-project");
  assert_eq!(window.text("project-name-note"), "Enter a project name.");
  window.set("project-name", "Kontur");
  assert_eq!(window.text("add-project-crumbs"), "All projects Set up Kontur");
  assert!(
    window.texts("project-name-note").is_none(),
    "a name the rules accept says nothing"
  );
  window.press("create-project");
  let workspace = window.launch.workspace().expect("the session").clone();
  window.until("the project to open", |window| {
    matches!(
      window.navigation().route_now(),
      Route::Project(_, ProjectPage::Overview(_))
    )
  });
  let projects = workspace.projects().expect("the list");
  assert_eq!(projects.len(), 1);
  assert_eq!(projects[0].repo_root, repo);
  assert_eq!(
    window.navigation().route_now(),
    Route::Project(projects[0].id, ProjectPage::Overview(OverviewTab::default()))
  );
}

#[test]
fn add_project_says_a_remote_is_not_on_github() {
  let scratch = scratch();
  let repo = repository(&scratch, "kontur");
  git(
    &repo,
    &["remote", "add", "origin", "https://gitlab.com/example/kontur.git"],
  );
  let (mut window, stand_ins) = add_project();
  choose(&mut window, &stand_ins, &repo);
  assert_eq!(window.text("github-detection-heading"), "Remote is not on GitHub");
  let block = window.text("github-detection");
  assert!(block.contains("Remote origin · gitlab.com/example/kontur"), "{block}");
  assert!(block.ends_with("Check again Add GitHub connection"), "{block}");
}

#[test]
fn add_project_refuses_a_folder_the_list_holds() {
  let scratch = scratch();
  let repo = repository(&scratch, "kontur");
  let (mut window, stand_ins) = add_project();
  let workspace = window.launch.workspace().expect("the session").clone();
  let held = orchester_workspace::project_setup::ProjectDraft {
    name: "Held",
    repository: Some(&repo),
  }
  .validate()
  .expect("a draft");
  workspace.create(held).expect("a project in the folder");
  choose(&mut window, &stand_ins, &repo);
  window.set("project-name", "Kontur");
  window.press("create-project");
  window.until("the failure", |window| window.texts("add-project-error").is_some());
  assert_eq!(
    window.text("add-project-error"),
    "Held in the list already uses this folder. Remove it from All projects first."
  );
  window.set("project-name", "Kontur 2");
  assert!(
    window.texts("add-project-error").is_none(),
    "a changed name retires the failure"
  );
}

#[test]
fn add_project_starts_with_the_folder_the_route_names() {
  let scratch = scratch();
  let repo = repository(&scratch, "kontur");
  let (mut window, _stand_ins) = Window::with_stand_ins(Data::new());
  window.navigation().go(Route::AddProject(RouteFolder::new(&repo)));
  window.until("the folder's detection", |window| {
    window
      .texts("github-detection-status")
      .is_some_and(|status| status == ["Local only"])
  });
  let shown = window.text("project-folder-value");
  assert!(shown.ends_with("kontur"), "{shown}");
}

#[test]
fn add_project_opens_within_the_home_tab() {
  let (mut window, _stand_ins) = add_project();
  let tabs = window.navigation().tabs_now();
  assert_eq!(tabs.tabs().len(), 1);
  let strip = window.texts("tab-strip").expect("the title bar's tabs");
  assert!(
    !strip.iter().any(|text| text == "Add project"),
    "the home tab shows its icon alone: {strip:?}"
  );
}

#[test]
fn add_project_in_the_only_tab_closes_into_all_projects() {
  let (window, _stand_ins) = add_project();
  let navigation = window.navigation();
  let tab = navigation.tabs_now().active().id();
  assert!(
    navigation.tabs_now().can_close(tab),
    "Add project is not the home itself"
  );
  navigation.close(tab);
  let tabs = navigation.tabs_now();
  assert_eq!(tabs.tabs().len(), 1);
  assert_eq!(*tabs.active().route(), Route::AllProjects);
  assert!(
    !tabs.can_close(tabs.active().id()),
    "All projects in the only tab stays"
  );
}

#[test]
fn add_project_back_returns_where_it_was_opened() {
  let (mut window, _stand_ins) = add_project();
  window.press("add-project-back");
  assert_eq!(window.navigation().route_now(), Route::AllProjects);
}
