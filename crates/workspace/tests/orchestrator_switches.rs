//! The Orchestrator's one switch, *Let the Orchestrator drive*, is each project's own (ORC-594, ORC-909): two projects
//! differ, a project with no value of its own takes the application-wide *answer agents* value it came from once when
//! it is first read, and a later application-wide value never reaches it; a person's revocation holds in its project
//! from the click. Nothing in the app sets the application-wide value any more: the tests write it as an older
//! Orchester left it.
//!
//! `cargo test -p orchester-workspace --test orchestrator_switches orchestrator_switches_`

use std::{
  fs,
  path::{Path, PathBuf},
  sync::{Arc, Barrier},
  thread,
};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{project_setup::NewProject, workspace::Workspace};
use rusqlite::Connection;
use tempfile::TempDir;

fn database(dir: &TempDir) -> PathBuf {
  dir.path().join("orchester.db")
}

fn workspace(dir: &TempDir) -> Workspace {
  Workspace::new(AppDb::open(&database(dir)).expect("an application database"))
}

/// A project `key` set up in the repository `repo-<key>` of `dir`, with no switch of its own.
fn add_project(dir: &TempDir, workspace: &Workspace, key: &str) -> PathBuf {
  let repo = dir.path().join(format!("repo-{key}"));
  fs::create_dir_all(repo.join(".git")).expect("a repository");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new(key).expect("a key"),
    Name::new(key).expect("a name"),
  );
  workspace
    .create(NewProject {
      project,
      repo_root: repo.clone(),
    })
    .expect("the project is set up");
  repo
}

/// The application-wide *answer agents* value, as an Orchester before ORC-909 stored it, written past the workspace.
fn application_wide(dir: &TempDir, allowed: bool) {
  AppDb::open(&database(dir)).expect("the application database");
  Connection::open(database(dir))
    .expect("the application database file")
    .execute(
      "UPDATE app_settings SET orchestrator_answers_agents = ?1 WHERE id = 1",
      [allowed],
    )
    .expect("saved");
}

fn stored(repo: &Path) -> Option<bool> {
  ProjectDb::open(repo)
    .expect("the project")
    .orchestrator_drives()
    .expect("read")
}

#[test]
fn orchestrator_switches_are_off_until_a_person_turns_it_on_in_its_project() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  assert!(!workspace.orchestrator_drives(&repo).expect("read"), "off by default");
  workspace
    .set_orchestrator_drives(&repo, true)
    .expect("the person turned it on");
  assert!(workspace.orchestrator_drives(&repo).expect("read"));
}

#[test]
fn orchestrator_switches_of_two_projects_differ() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (kontur, other) = (
    add_project(&dir, &workspace, "KON"),
    add_project(&dir, &workspace, "OTH"),
  );
  workspace.set_orchestrator_drives(&kontur, true).expect("on in Kontur");
  workspace
    .set_orchestrator_drives(&other, false)
    .expect("off in the other");
  assert!(workspace.orchestrator_drives(&kontur).expect("read"));
  assert!(!workspace.orchestrator_drives(&other).expect("read"));
  assert_eq!(stored(&kontur), Some(true));
  assert_eq!(stored(&other), Some(false));
  let application_wide = AppDb::open(&database(&dir)).expect("the application database");
  assert!(
    !application_wide.orchestrator_answers_agents().expect("read"),
    "a project's choice leaves the application-wide value as it was"
  );
}

#[test]
fn orchestrator_switches_of_a_project_without_its_own_are_copied_once_from_the_application_wide_one() {
  let dir = TempDir::new().expect("a folder");
  application_wide(&dir, true);
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  assert_eq!(stored(&repo), None, "nothing of its own yet");
  assert!(
    workspace.orchestrator_drives(&repo).expect("read"),
    "the project drives where the application-wide Answer agents was on"
  );
  assert_eq!(stored(&repo), Some(true), "copied into the project");
}

#[test]
fn orchestrator_switches_turned_on_application_wide_later_do_not_reach_a_project() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  assert!(!workspace.orchestrator_drives(&repo).expect("read"));

  application_wide(&dir, true);
  assert!(
    !workspace.orchestrator_drives(&repo).expect("read"),
    "a later application-wide change does not reach the project"
  );
  assert_eq!(stored(&repo), Some(false));
  // A project added after the change copies the application-wide value it finds then.
  let later = add_project(&dir, &workspace, "NEW");
  assert!(workspace.orchestrator_drives(&later).expect("read"));
}

#[test]
fn orchestrator_switches_are_copied_once_when_the_project_opens() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  workspace.open(&repo).expect("opened");
  assert_eq!(stored(&repo), Some(false), "copied before anything reads it");
  application_wide(&dir, true);
  workspace.open(&repo).expect("opened again");
  assert_eq!(stored(&repo), Some(false), "the copy is made once");
  assert!(!workspace.orchestrator_drives(&repo).expect("read"));
}

#[test]
fn orchestrator_switches_a_project_set_itself_are_never_copied_over() {
  let dir = TempDir::new().expect("a folder");
  application_wide(&dir, true);
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  workspace
    .set_orchestrator_drives(&repo, false)
    .expect("off for the project");
  workspace.open(&repo).expect("opened");
  assert!(
    !workspace.orchestrator_drives(&repo).expect("read"),
    "the project's own value is not replaced"
  );
  assert_eq!(stored(&repo), Some(false));
}

#[test]
fn orchestrator_switches_are_off_from_a_persons_revocation_in_its_project_only() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (kontur, other) = (
    add_project(&dir, &workspace, "KON"),
    add_project(&dir, &workspace, "OTH"),
  );
  for repo in [&kontur, &other] {
    workspace
      .set_orchestrator_drives(repo, true)
      .expect("the person turned it on");
  }
  // The person's click in Kontur; its save has not run yet.
  workspace.revoke_orchestrator_drive(&kontur);
  assert!(
    !workspace.orchestrator_drives(&kontur).expect("read"),
    "off from the person's click, though the stored value is still on"
  );
  assert_eq!(stored(&kontur), Some(true));
  assert!(
    workspace.orchestrator_drives(&other).expect("read"),
    "the other project's switch stays on"
  );
  workspace
    .set_orchestrator_drives(&kontur, true)
    .expect("turned on again");
  assert!(workspace.orchestrator_drives(&kontur).expect("read"));
}

#[test]
fn orchestrator_switches_reads_that_overlap_each_report_the_stored_grant() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  workspace
    .set_orchestrator_drives(&repo, true)
    .expect("the person turned it on");
  let started = Arc::new(Barrier::new(4));
  let reads = [(); 4].map(|()| {
    let (reader, repo, started) = (workspace.clone(), repo.clone(), Arc::clone(&started));
    thread::spawn(move || {
      started.wait();
      reader.orchestrator_drives(&repo)
    })
  });
  for read in reads {
    assert!(
      read.join().expect("reader joins").expect("the setting reads"),
      "a read that overlaps another still reports the person's stored grant"
    );
  }
}
