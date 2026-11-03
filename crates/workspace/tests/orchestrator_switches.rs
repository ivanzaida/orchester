//! The Orchestrator's switches are each project's own (ORC-594): two projects differ, a project with no value of its
//! own takes the application-wide one once when it is first read, and a later application-wide value never reaches
//! it; a person's revocation holds in its project from the click. Nothing in the app sets the application-wide values
//! any more (ORC-600): the tests write them as an older Orchester left them.
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
use orchester_store::{AppDb, OrchestratorSwitch, ProjectDb};
use orchester_workspace::{
  project_setup::NewProject,
  workspace::{OrchestratorConsent, Workspace},
};
use tempfile::TempDir;

const CONSENTS: [(OrchestratorConsent, OrchestratorSwitch); 6] = [
  (OrchestratorConsent::PublishDrafts, OrchestratorSwitch::PublishDrafts),
  (OrchestratorConsent::AnswerAgents, OrchestratorSwitch::AnswerAgents),
  (OrchestratorConsent::MergeRuns, OrchestratorSwitch::MergeRuns),
  (OrchestratorConsent::MergeFinished, OrchestratorSwitch::MergeFinished),
  (
    OrchestratorConsent::ContinueInterrupted,
    OrchestratorSwitch::ContinueInterrupted,
  ),
  (OrchestratorConsent::WriteGit, OrchestratorSwitch::WriteGit),
];

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

/// The application-wide value of every switch, as an Orchester before ORC-594 stored it, written past the workspace.
fn application_wide(dir: &TempDir, allowed: bool) {
  let mut db = AppDb::open(&database(dir)).expect("the application database");
  db.set_orchestrator_publishes_drafts(allowed).expect("saved");
  db.set_orchestrator_answers_agents(allowed).expect("saved");
  db.set_orchestrator_merges_runs(allowed).expect("saved");
  db.set_orchestrator_merges_finished(allowed).expect("saved");
  db.set_orchestrator_continues_interrupted(allowed).expect("saved");
  db.set_orchestrator_writes_git(allowed).expect("saved");
}

/// Whether any application-wide value is on.
fn any_application_wide(dir: &TempDir) -> bool {
  let db = AppDb::open(&database(dir)).expect("the application database");
  [
    db.orchestrator_publishes_drafts(),
    db.orchestrator_answers_agents(),
    db.orchestrator_merges_runs(),
    db.orchestrator_merges_finished(),
    db.orchestrator_continues_interrupted(),
    db.orchestrator_writes_git(),
  ]
  .into_iter()
  .any(|read| read.expect("read"))
}

fn stored(repo: &Path, switch: OrchestratorSwitch) -> Option<bool> {
  ProjectDb::open(repo)
    .expect("the project")
    .orchestrator_switch(switch)
    .expect("read")
}

#[test]
fn orchestrator_switches_are_off_until_a_person_turns_each_on_in_its_project() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  for (consent, _) in CONSENTS {
    assert!(
      !workspace.orchestrator_allows(&repo, consent).expect("read"),
      "{consent:?} is off by default"
    );
  }
  workspace
    .set_orchestrator_allows(&repo, OrchestratorConsent::AnswerAgents, true)
    .expect("the person turned it on");
  for (consent, _) in CONSENTS {
    assert_eq!(
      workspace.orchestrator_allows(&repo, consent).expect("read"),
      consent == OrchestratorConsent::AnswerAgents,
      "{consent:?}: each switch is its own"
    );
  }
}

#[test]
fn orchestrator_switches_of_two_projects_differ() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (kontur, other) = (
    add_project(&dir, &workspace, "KON"),
    add_project(&dir, &workspace, "OTH"),
  );
  for (consent, switch) in CONSENTS {
    workspace
      .set_orchestrator_allows(&kontur, consent, true)
      .expect("on in Kontur");
    workspace
      .set_orchestrator_allows(&other, consent, false)
      .expect("off in the other");
    assert!(workspace.orchestrator_allows(&kontur, consent).expect("read"));
    assert!(!workspace.orchestrator_allows(&other, consent).expect("read"));
    assert_eq!(stored(&kontur, switch), Some(true), "{consent:?}");
    assert_eq!(stored(&other, switch), Some(false), "{consent:?}");
  }
  assert!(
    !any_application_wide(&dir),
    "a project's choice leaves the application-wide values as they were"
  );
}

#[test]
fn orchestrator_switches_of_a_project_without_its_own_are_copied_once_from_the_application_wide_ones() {
  let dir = TempDir::new().expect("a folder");
  application_wide(&dir, true);
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  for (consent, switch) in CONSENTS {
    assert_eq!(stored(&repo, switch), None, "{consent:?}: nothing of its own yet");
    assert!(
      workspace.orchestrator_allows(&repo, consent).expect("read"),
      "{consent:?}: the project keeps the application-wide choice it had"
    );
    assert_eq!(
      stored(&repo, switch),
      Some(true),
      "{consent:?}: copied into the project"
    );
  }
}

#[test]
fn orchestrator_switches_turned_on_application_wide_later_do_not_reach_a_project() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  for (consent, _) in CONSENTS {
    assert!(!workspace.orchestrator_allows(&repo, consent).expect("read"));
  }

  application_wide(&dir, true);
  for (consent, switch) in CONSENTS {
    assert!(
      !workspace.orchestrator_allows(&repo, consent).expect("read"),
      "{consent:?}: a later application-wide change does not reach the project"
    );
    assert_eq!(stored(&repo, switch), Some(false), "{consent:?}");
  }
  // A project added after the change copies the application-wide value it finds then.
  let later = add_project(&dir, &workspace, "NEW");
  for (consent, _) in CONSENTS {
    assert!(workspace.orchestrator_allows(&later, consent).expect("read"));
  }
}

#[test]
fn orchestrator_switches_are_copied_once_when_the_project_opens() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  workspace.open(&repo).expect("opened");
  for (consent, switch) in CONSENTS {
    assert_eq!(
      stored(&repo, switch),
      Some(false),
      "{consent:?}: copied before anything reads it"
    );
  }
  application_wide(&dir, true);
  workspace.open(&repo).expect("opened again");
  for (consent, switch) in CONSENTS {
    assert_eq!(stored(&repo, switch), Some(false), "{consent:?}: the copy is made once");
    assert!(!workspace.orchestrator_allows(&repo, consent).expect("read"));
  }
}

#[test]
fn orchestrator_switches_a_project_set_itself_are_never_copied_over() {
  let dir = TempDir::new().expect("a folder");
  application_wide(&dir, true);
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  for (consent, switch) in CONSENTS {
    workspace
      .set_orchestrator_allows(&repo, consent, false)
      .expect("off for the project");
    workspace.open(&repo).expect("opened");
    assert!(
      !workspace.orchestrator_allows(&repo, consent).expect("read"),
      "{consent:?}: the project's own value is not replaced"
    );
    assert_eq!(stored(&repo, switch), Some(false));
  }
}

#[test]
fn orchestrator_switches_are_off_from_a_persons_revocation_in_its_project_only() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (kontur, other) = (
    add_project(&dir, &workspace, "KON"),
    add_project(&dir, &workspace, "OTH"),
  );
  for (consent, switch) in CONSENTS {
    for repo in [&kontur, &other] {
      workspace
        .set_orchestrator_allows(repo, consent, true)
        .expect("the person turned it on");
    }
    // The person's click in Kontur; its save has not run yet.
    workspace.revoke_orchestrator_consent(&kontur, consent);
    assert!(
      !workspace.orchestrator_allows(&kontur, consent).expect("read"),
      "{consent:?}: off from the person's click, though the stored value is still on"
    );
    assert_eq!(stored(&kontur, switch), Some(true));
    assert!(
      workspace.orchestrator_allows(&other, consent).expect("read"),
      "{consent:?}: the other project's switch stays on"
    );
    workspace
      .set_orchestrator_allows(&kontur, consent, true)
      .expect("turned on again");
    assert!(workspace.orchestrator_allows(&kontur, consent).expect("read"));
  }
}

#[test]
fn orchestrator_switches_reads_that_overlap_each_report_the_stored_grant() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let repo = add_project(&dir, &workspace, "KON");
  let consent = OrchestratorConsent::AnswerAgents;
  workspace
    .set_orchestrator_allows(&repo, consent, true)
    .expect("the person turned it on");
  let started = Arc::new(Barrier::new(4));
  let reads = [(); 4].map(|()| {
    let (reader, repo, started) = (workspace.clone(), repo.clone(), Arc::clone(&started));
    thread::spawn(move || {
      started.wait();
      reader.orchestrator_allows(&repo, consent)
    })
  });
  for read in reads {
    assert!(
      read.join().expect("reader joins").expect("the setting reads"),
      "a read that overlaps another still reports the person's stored grant"
    );
  }
}
