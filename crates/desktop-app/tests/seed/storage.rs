//! Storage fixtures leave real readable folders and a real low-disk guard for UI verification.
use orchester_core::workspaces::KeepFree;
use orchester_workspace::storage::{self, KeptKind};

use super::Seed;

#[test]
fn seed_storage_low_disk_keeps_readable_ended_and_paused_worktrees_and_a_warm_workspace() {
  let seed = Seed::of("storage-low-disk");
  let disk = storage::guard_local(seed.repo()).expect("disk guard");
  assert_eq!(disk.keep_free.gigabytes(), KeepFree::MAX_GB);
  assert!(disk.holds());
  assert_eq!(storage::paused_runs(seed.repo()).expect("paused runs"), 1);
  let rows = storage::inventory(seed.repo()).expect("readable inventory");
  assert_eq!(rows.len(), 4);
  assert_eq!(rows.iter().filter_map(storage::KeptRow::removable_run).count(), 2);
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
  assert!(storage::inventory(seed.repo()).expect("inventory").is_empty());
}
