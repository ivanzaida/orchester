//! Storage fixtures leave real readable folders and a real low-disk guard for UI verification; the project's real
//! runner lists them, as the Storage page reads them (`docs/architecture/remote-runners.md`, rule 5).
use orchester_core::workspaces::KeepFree;
use orchester_ipc::SessionEnd;
use orchester_workspace::{
  activity::Activity,
  runner::{ExecutorFolders, QUIET_STOP_TIMEOUT, Runners},
  storage::{self, KeptKind, KeptRow},
  workspace::OpenProject,
};

use super::{Seed, runner_programs::programs};

/// The Storage page's inventory of `seed`, listed by the project's real runner.
fn inventory(seed: &Seed) -> Vec<KeptRow> {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let runners = Runners::new(
    programs().runner.clone(),
    runtime.handle().clone(),
    Activity::new(),
    SessionEnd::Clean,
  );
  let project = OpenProject {
    project: seed.db().project().clone(),
    repo_root: seed.repo().to_path_buf(),
  };
  let folders = ExecutorFolders::new(runners.clone(), project);
  let rows = storage::inventory(seed.repo(), &folders).expect("readable inventory");
  runtime.block_on(runners.shutdown(Some(QUIET_STOP_TIMEOUT)));
  rows
}

#[test]
fn seed_storage_low_disk_keeps_readable_ended_and_paused_worktrees_and_a_warm_workspace() {
  let seed = Seed::of("storage-low-disk");
  let disk = storage::guard_local(seed.repo()).expect("disk guard");
  assert_eq!(disk.keep_free.gigabytes(), KeepFree::MAX_GB);
  assert!(disk.holds());
  assert_eq!(storage::paused_runs(seed.repo()).expect("paused runs"), 1);
  let rows = inventory(&seed);
  assert_eq!(rows.len(), 4);
  assert_eq!(rows.iter().filter_map(KeptRow::removable_run).count(), 2);
  assert_eq!(
    rows
      .iter()
      .filter(|row| matches!(row.kind, KeptKind::Workspace(_)))
      .count(),
    1
  );
}

#[test]
fn seed_storage_empty_has_no_kept_folders() {
  let seed = Seed::of("storage-empty");
  assert!(inventory(&seed).is_empty());
}
