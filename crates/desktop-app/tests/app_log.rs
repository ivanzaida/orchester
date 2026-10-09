//! The app's log: one file per UTC day, the last 14 days kept, other files left alone.

use std::{
  fs,
  time::{Duration, SystemTime},
};

use orchester_app::app_log::{AppLog, KEPT_DAYS, log_path, prune, utc_date};

const DAY: Duration = Duration::from_secs(86_400);

/// 2026-10-09 12:00 UTC.
fn noon() -> SystemTime {
  SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_547_200)
}

#[test]
fn each_utc_day_has_its_own_file() {
  let dir = tempfile::tempdir().expect("a temporary folder");
  assert_eq!(utc_date(noon()), "2026-10-09");
  assert_eq!(
    log_path(dir.path(), noon()).file_name().and_then(|name| name.to_str()),
    Some("orchester-app-2026-10-09.log")
  );
  let log = AppLog::open(dir.path(), noon()).expect("the log opens");
  log.write_at(noon(), b"first\n").expect("a line is written");
  log.write_at(noon() + DAY, b"second\n").expect("a line is written");
  let today = fs::read_to_string(log_path(dir.path(), noon())).expect("today's file");
  let tomorrow = fs::read_to_string(log_path(dir.path(), noon() + DAY)).expect("tomorrow's file");
  assert_eq!((today.as_str(), tomorrow.as_str()), ("first\n", "second\n"));
}

#[test]
fn old_files_are_deleted_and_others_left_alone() {
  let dir = tempfile::tempdir().expect("a temporary folder");
  let oldest_kept = noon() - DAY * (KEPT_DAYS - 1);
  let too_old = noon() - DAY * KEPT_DAYS;
  for at in [noon(), oldest_kept, too_old] {
    fs::write(log_path(dir.path(), at), "").expect("a log file");
  }
  let foreign = dir.path().join("orchester-2000-01-01.log");
  fs::write(&foreign, "").expect("another app's file");
  prune(dir.path(), noon()).expect("old files are deleted");
  assert!(log_path(dir.path(), noon()).exists());
  assert!(log_path(dir.path(), oldest_kept).exists());
  assert!(!log_path(dir.path(), too_old).exists());
  assert!(foreign.exists(), "only this app's log files are touched");
}
