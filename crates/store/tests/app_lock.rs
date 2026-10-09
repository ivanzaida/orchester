//! The data folder's lock: one Orchester app at a time per data folder, and who holds it.

use std::fs;

use orchester_core::time::Timestamp;
use orchester_store::location::{AppLock, AppLockError, LockHolder};

fn holder(pid: u32) -> LockHolder {
  LockHolder {
    pid,
    started_at: Timestamp::from_unix_millis(1_760_000_000_000),
  }
}

#[test]
fn app_lock_keeps_a_second_app_out_and_says_who_holds_it() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let database = dir.path().join("orchester.db");

  let _held = AppLock::acquire(&database, holder(18244)).expect("the first app takes the lock");
  let Err(AppLockError::InUse { folder, holder: found }) = AppLock::acquire(&database, holder(2)) else {
    panic!("a second app finds the data folder in use");
  };
  assert_eq!(folder, dir.path());
  assert_eq!(found, Some(holder(18244)));
}

#[test]
fn app_lock_is_free_once_its_holder_lets_go() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let database = dir.path().join("orchester.db");

  drop(AppLock::acquire(&database, holder(1)).expect("the first app takes the lock"));
  let _second = AppLock::acquire(&database, holder(2)).expect("the next app takes it after the first closed");
  let Err(AppLockError::InUse { holder: found, .. }) = AppLock::acquire(&database, holder(3)) else {
    panic!("the second app holds the folder now");
  };
  assert_eq!(
    found,
    Some(holder(2)),
    "the record names the app that holds the lock now"
  );
}

#[test]
fn app_lock_without_its_holders_record_still_keeps_others_out() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let database = dir.path().join("orchester.db");

  let _held = AppLock::acquire(&database, holder(1)).expect("the first app takes the lock");
  fs::remove_file(dir.path().join("orchester.lock.holder")).expect("the holder's record is removed");
  let Err(AppLockError::InUse { holder: found, .. }) = AppLock::acquire(&database, holder(2)) else {
    panic!("the lock alone keeps the folder in use");
  };
  assert_eq!(found, None);
}

#[test]
fn app_lock_reports_a_data_folder_it_cannot_write() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let folder = dir.path().join("missing");

  let Err(AppLockError::Io { folder: reported, .. }) = AppLock::acquire(&folder.join("orchester.db"), holder(1)) else {
    panic!("no lock can be written in a folder that is not there");
  };
  assert_eq!(reported, folder);
}
