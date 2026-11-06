//! What the recovery shows of each interrupted run's folder, as the project's real runner reads it there with git:
//! the files not committed and the first of them, a clean folder, or a folder that is gone. The runner is started for
//! the recovery, as no tab shows the project.

use std::{fs, path::Path, process::Command, time::Duration};

use orchester_core::{id::RunId, integration::WORKTREES_DIR};
use orchester_ipc::SessionEnd;
use orchester_store::AppDb;
use orchester_workspace::{
  activity::Activity,
  recovery::{Reading, Recovery, WorktreeState},
  runner::{QUIET_STOP_TIMEOUT, Runners},
  workspace::Workspace,
};
use tokio::runtime::Runtime;

use super::{
  runner_programs::programs,
  support::{Fixture, at},
};

/// How long the runner may take to start, attach and read the folders.
const PATIENCE: Duration = Duration::from_secs(60);

fn git(folder: &Path, args: &[&str]) {
  let status = Command::new("git")
    .args(args)
    .current_dir(folder)
    .status()
    .expect("git runs");
  assert!(status.success(), "git {args:?} in {}", folder.display());
}

/// `run`'s own worktree in `repo`, made a repository of its own with nothing committed.
fn worktree(repo: &Path, run: RunId) -> std::path::PathBuf {
  let folder = repo.join(WORKTREES_DIR).join(run.to_string());
  fs::create_dir_all(&folder).expect("the run's worktree");
  git(&folder, &["init", "--quiet"]);
  folder
}

#[test]
fn recovery_folder_states_are_read_by_the_runner_as_git_reports_them() {
  let mut fixture = Fixture::new();
  let tasks = [
    fixture.task("Write docs", at(10)),
    fixture.task("Add login", at(11)),
    fixture.task("Plan zoom", at(12)),
  ];
  let [dirty, clean, gone] = tasks.map(|task| fixture.started_run(task, at(20)).id());
  let dirty_folder = worktree(fixture.repo.path(), dirty);
  fs::create_dir(dirty_folder.join("notes")).expect("a folder in the worktree");
  fs::write(dirty_folder.join("notes").join("b.md"), "draft").expect("an untracked file");
  fs::write(dirty_folder.join("a.txt"), "work").expect("another untracked file");
  worktree(fixture.repo.path(), clean);
  let project = fixture.db.project().id();

  let app = tempfile::tempdir().expect("an app folder");
  let workspace = Workspace::new(AppDb::open(&app.path().join("orchester.db")).expect("the app database"));
  workspace.open(fixture.repo.path()).expect("the project is registered");
  let runtime = Runtime::new().expect("a runtime");
  let runners = Runners::new(
    programs().runner.clone(),
    runtime.handle().clone(),
    Activity::new(),
    SessionEnd::Crashed,
  );
  let recovery = Recovery::start(
    (workspace, runners.clone()),
    SessionEnd::Crashed,
    at(90),
    runtime.handle(),
  );
  let mut updates = recovery.subscribe();
  let state = runtime.block_on(async {
    let read = tokio::time::timeout(PATIENCE, updates.wait_for(|state| state.reading != Reading::Reading));
    read
      .await
      .expect("the recovery reads in time")
      .expect("the recovery reads")
      .clone()
  });
  let states: Vec<_> = state
    .runs_of(project)
    .into_iter()
    .map(|listed| {
      (
        listed.run,
        listed.worktree,
        listed.uncommitted_file.as_deref().map(str::to_owned),
      )
    })
    .collect();
  assert_eq!(
    states,
    [
      (dirty, WorktreeState::Uncommitted(2), Some("a.txt".to_owned())),
      (clean, WorktreeState::Clean, None),
      (gone, WorktreeState::Unknown, None),
    ]
  );
  runtime.block_on(runners.shutdown(Some(QUIET_STOP_TIMEOUT)));
}
