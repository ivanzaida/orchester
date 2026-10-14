//! All projects' rules: when a session welcomes the person, where each listed project stands with its
//! runs, and removing a project from the list, which never touches its folder or its data.

use std::{fs, path::PathBuf, sync::Arc};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  run::{RunCounts, TaskPhase},
  time::Timestamp,
};
use orchester_ipc::SessionEnd;
use orchester_store::{AppDb, RegisteredProject};
use orchester_workspace::{
  activity::Activity,
  messages::Message,
  project_list::{self, ListedProject, ListedRuns, Listing, RunnerShown, Standing},
  project_setup::NewProject,
  runner::{
    Runners,
    state::{RunnerFailure, RunnerState},
  },
  workspace::{ProjectData, ProjectEntry, Workspace},
};
use tempfile::TempDir;

fn workspace(dir: &TempDir) -> Workspace {
  Workspace::new(AppDb::open(&dir.path().join("orchester.db")).expect("an application database"))
}

/// Kontur set up in the repository `repo-kontur` of `dir`.
fn add_kontur(dir: &TempDir, workspace: &Workspace) -> RegisteredProject {
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
      repo_root: repo,
    })
    .expect("the project is set up");
  workspace.projects().expect("the list").remove(0)
}

fn entry(folder_missing: bool) -> ProjectEntry {
  ProjectEntry {
    project: RegisteredProject {
      id: ProjectId::generate(),
      name: Name::new("Kontur").expect("a name"),
      repo_root: PathBuf::from("kontur"),
      last_opened_at: Timestamp::from_unix_millis(0),
    },
    folder_missing,
  }
}

/// A project whose tasks' latest runs are in `phases`, and where two things wait for the person.
fn readable(phases: &[TaskPhase]) -> ProjectData {
  ProjectData::Readable(Some(ListedRuns::new(Arc::from(phases), (2, 2))))
}

#[test]
fn project_list_welcomes_a_session_without_projects_until_it_listed_one() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  assert_eq!(workspace.listing().expect("the list reads"), Listing::Welcome);
  let kontur = add_kontur(&dir, &workspace);
  let Listing::Projects(entries) = workspace.listing().expect("the list reads") else {
    panic!("a listed project is not a welcome");
  };
  assert_eq!(entries.len(), 1);
  workspace.forget(kontur.id).expect("removed from the list");
  assert_eq!(
    workspace.listing().expect("the list reads"),
    Listing::Projects(Vec::new()),
    "every project removed is an empty list, not a first launch"
  );
}

#[test]
fn project_list_rows_say_where_each_project_stands() {
  let phases = [
    TaskPhase::Running,
    TaskPhase::Waiting,
    TaskPhase::Stuck,
    TaskPhase::Draft,
  ];
  let online = ListedProject::of(
    &entry(false),
    Some(&readable(&phases)),
    Some((&RunnerState::Online, Some(1))),
  );
  assert_eq!(online.standing, Standing::Opens(RunnerShown::Online));
  assert_eq!(
    online.counts,
    Some(RunCounts {
      active: 1,
      needs_you: 2
    })
  );
  assert!(online.opens());

  let not_started = ListedProject::of(&entry(false), Some(&readable(&phases)), None);
  assert_eq!(not_started.standing, Standing::Opens(RunnerShown::NotStarted));
  assert_eq!(
    not_started.counts.map(|counts| counts.active),
    Some(0),
    "no agent works without a runner online"
  );

  let failed = RunnerState::Failed {
    cause: RunnerFailure::StartFailed,
    attempts: 0,
  };
  let offline = ListedProject::of(&entry(false), None, Some((&failed, Some(0))));
  assert_eq!(offline.standing, Standing::Opens(RunnerShown::Offline));
  assert_eq!(
    offline.counts, None,
    "a dash until the counts are read, never a made-up 0"
  );

  let missing = ListedProject::of(&entry(true), Some(&readable(&phases)), None);
  assert_eq!(missing.standing, Standing::FolderMissing);
  assert_eq!((missing.counts, missing.opens()), (None, false));

  let reason = Message::new("errors.newerSchema");
  let unreadable = ListedProject::of(&entry(false), Some(&ProjectData::Unreadable(reason.clone())), None);
  assert_eq!(unreadable.standing, Standing::Unreadable(reason));
  assert!(!unreadable.opens());
}

#[test]
fn project_list_removal_is_refused_while_agents_work() {
  let working = ListedProject::of(&entry(false), None, Some((&RunnerState::Online, Some(2))));
  assert_eq!(
    working.removal_refused(),
    Some(Message::new("projects.removeRefused").with("name", "Kontur".to_owned()))
  );
  let idle = ListedProject::of(&entry(false), None, Some((&RunnerState::Online, Some(0))));
  assert_eq!(idle.removal_refused(), None);
  for unknown in [RunnerState::Starting, RunnerState::Online, RunnerState::Stopping] {
    let row = ListedProject::of(&entry(false), None, Some((&unknown, None)));
    assert_eq!(
      row.removal_refused(),
      Some(Message::new("projects.removeUnknown").with("name", "Kontur".to_owned())),
      "{unknown:?} without a report of its work"
    );
  }
}

#[test]
fn project_list_remove_forgets_only_the_list_entry() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let kontur = add_kontur(&dir, &workspace);
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let runners = Runners::new(
    dir.path().join("no-runner"),
    runtime.handle().clone(),
    Activity::new(),
    SessionEnd::Clean,
  );
  runners.ensure(&kontur.repo_root, "Kontur");
  // The runner program is missing: once it failed, nothing runs and the project can go.
  runtime.block_on(async {
    let mut updates = runners.work_changes();
    let failed = async {
      while !matches!(
        runners
          .status_of(&kontur.repo_root)
          .map(|status| status.state().clone()),
        Some(RunnerState::Failed { .. })
      ) {
        updates.changed().await.expect("the runners outlive the test");
      }
    };
    tokio::time::timeout(std::time::Duration::from_secs(20), failed)
      .await
      .expect("the runner fails in time");
  });
  runtime
    .block_on(project_list::remove(&workspace, &runners, &kontur))
    .expect("removed from the list");
  assert!(
    runners.status_of(&kontur.repo_root).is_none(),
    "its runner is forgotten"
  );
  assert!(workspace.projects().expect("the list reads").is_empty());
  assert!(
    kontur.repo_root.join(".orchester").is_dir(),
    "the project's data stays on disk"
  );
  assert!(
    workspace.open(&kontur.repo_root).is_ok(),
    "opening the folder brings it back"
  );
}
