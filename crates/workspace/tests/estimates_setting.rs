//! *Let Orchester estimate story points* is each project's own (ORC-103): opening a project that has no value of its
//! own copies the application-wide one it moved from into it, once (ORC-595); afterwards only the project's is read.
//!
//! `cargo test -p orchester-workspace --test estimates_setting estimates_setting_`

use std::{fs, path::PathBuf};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{project_setup::NewProject, workspace::Workspace};
use tempfile::TempDir;

fn database(dir: &TempDir) -> PathBuf {
  dir.path().join("orchester.db")
}

/// A workspace whose application-wide setting of before ORC-103 is `allowed`.
fn workspace(dir: &TempDir, allowed: bool) -> Workspace {
  let mut db = AppDb::open(&database(dir)).expect("an application database");
  db.set_estimates_story_points(allowed).expect("the old setting");
  Workspace::new(db)
}

/// Kontur set up in the repository `repo-kontur` of `dir`, as a project of before ORC-103 with no value of its own.
fn add_kontur(dir: &TempDir, workspace: &Workspace) -> PathBuf {
  let repo = dir.path().join("repo-kontur");
  fs::create_dir_all(repo.join(".git")).expect("a repository");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("a key"),
    Name::new("Kontur").expect("a name"),
  );
  workspace
    .create(NewProject {
      project,
      repo_root: repo.clone(),
    })
    .expect("the project is set up");
  repo
}

fn application_wide(dir: &TempDir, allowed: bool) {
  AppDb::open(&database(dir))
    .expect("the application database")
    .set_estimates_story_points(allowed)
    .expect("the old setting");
}

#[test]
fn estimates_setting_of_a_project_without_its_own_is_copied_once_when_it_opens() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir, true);
  let repo = add_kontur(&dir, &workspace);
  workspace.open(&repo).expect("opened");
  assert!(
    workspace.estimates_story_points(&repo).expect("read"),
    "the project keeps the application-wide setting it had"
  );
  assert!(
    ProjectDb::open(&repo)
      .expect("the project")
      .estimates_story_points()
      .expect("read")
  );

  application_wide(&dir, false);
  assert!(
    workspace.estimates_story_points(&repo).expect("read"),
    "only the project's value is read once it is copied"
  );
  workspace.open(&repo).expect("opened again");
  assert!(
    workspace.estimates_story_points(&repo).expect("read"),
    "the copy is made once"
  );
}

#[test]
fn estimates_setting_a_project_set_itself_stays_when_it_opens() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir, true);
  let repo = add_kontur(&dir, &workspace);
  workspace
    .set_estimates_story_points(&repo, false)
    .expect("off for the project");
  workspace.open(&repo).expect("opened");
  assert!(
    !workspace.estimates_story_points(&repo).expect("read"),
    "the project's own value is not replaced"
  );
}
