//! Which merges Orchester tries again by itself, and when (ORC-365, `orchester_core::integration::retry`): causes that
//! pass by themselves are tried again three times, 10 s and then 30 s apart; lasting ones wait for a person.
//! `cargo test -p orchester-core --test integration merge_retry`.

use std::{io, num::NonZeroU32, time::Duration};

use orchester_core::integration::{
  GitCause, GitError, MergeError, PreflightCause,
  local::LocalError,
  retry::{RETRIES, delay_before, held_by_another_program},
};

/// The OS's error number of a file another program holds: a sharing violation on Windows, `EBUSY` on Unix.
#[cfg(windows)]
const HELD_FILE: i32 = 32;
#[cfg(unix)]
const HELD_FILE: i32 = 16;
/// `ERROR_CRC` on Windows, `EIO` on Unix: the disk failed.
#[cfg(windows)]
const DISK_FAILED: i32 = 23;
#[cfg(unix)]
const DISK_FAILED: i32 = 5;

fn io_cause(code: i32) -> PreflightCause {
  PreflightCause::Io {
    path: Some(".orchester/orchester.db".to_owned()),
    kind: io::ErrorKind::Other,
    code: Some(code),
    text: "the OS's words".to_owned(),
  }
}

#[test]
fn merge_retry_tries_three_times_ten_then_thirty_seconds_apart() {
  assert_eq!(RETRIES, 3);
  let delays: Vec<Duration> = (1..=RETRIES).filter_map(NonZeroU32::new).map(delay_before).collect();
  assert_eq!(
    delays,
    [
      Duration::from_secs(10),
      Duration::from_secs(30),
      Duration::from_secs(30)
    ]
  );
}

#[test]
fn merge_retry_passing_causes_are_a_held_lease_a_held_file_and_a_locked_index() {
  assert!(PreflightCause::Held.passes());
  assert!(
    PreflightCause::State(LocalError::ProcessUncertain).passes(),
    "Git programs not proved stopped yet"
  );
  assert!(held_by_another_program(HELD_FILE));
  assert!(io_cause(HELD_FILE).passes());
  assert!(MergeError::Refused("error: unable to unlink old 'a.txt': Device or resource busy".to_owned()).passes());
  assert!(
    MergeError::Git(GitError::Failed(
      "fatal: Unable to create 'index.lock': File exists.\nAnother git process seems to be running".to_owned()
    ))
    .passes()
  );
}

#[test]
fn merge_retry_lasting_causes_wait_for_a_person() {
  assert!(!io_cause(DISK_FAILED).passes(), "a disk's I/O error");
  assert!(!PreflightCause::Git(GitCause::new(Some(128), b"fatal: not a git repository")).passes());
  assert!(
    !PreflightCause::State(LocalError::Unsupported).passes(),
    "a checkout that is not the one recorded"
  );
  assert!(!MergeError::Refused("pre-merge-commit hook refused".to_owned()).passes());
  assert!(
    !MergeError::Conflict {
      files: vec!["a.txt".to_owned()]
    }
    .passes()
  );
  assert!(
    !MergeError::LeftChanged {
      files: vec!["a.txt".to_owned()],
      detail: "Device or resource busy".to_owned(),
    }
    .passes(),
    "files left changed wait to be put back"
  );
}
