//! The apps' daily log: one file per UTC day, appended to across starts, the next day's file begun at
//! midnight, and only the files of the last `KEPT_DAYS` days with its own prefix kept.

use std::{
  fs,
  path::{Path, PathBuf},
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use orchester_ipc::daily_log::{self, DailyLog, KEPT_DAYS};
use tempfile::TempDir;

const DAY: Duration = Duration::from_secs(86_400);
const PREFIX: &str = "orchester-";

fn log_path(dir: &Path, at: SystemTime) -> PathBuf {
  daily_log::log_path(dir, PREFIX, at)
}

/// 2026-09-27 at `hour`:00 UTC; hour 24 is midnight of the 28th.
fn sep_27(hour: u64) -> SystemTime {
  UNIX_EPOCH + Duration::from_secs(1_790_467_200 + hour * 3_600)
}

/// A temporary application directory and its log folder, not created yet.
fn logs() -> (TempDir, PathBuf) {
  let app = tempfile::tempdir().expect("temporary directory");
  let logs = app.path().join("logs");
  (app, logs)
}

fn read(path: &Path) -> String {
  fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn daily_log_writes_to_the_file_of_the_day() {
  let (_app, logs) = logs();
  let log = DailyLog::open(&logs, PREFIX, sep_27(9)).expect("open the log");
  log.write_at(sep_27(9), b"started\n").expect("write");

  let today = logs.join("orchester-2026-09-27.log");
  assert_eq!(log_path(&logs, sep_27(9)), today);
  assert_eq!(read(&today), "started\n");
}

#[test]
fn daily_log_appends_across_starts_on_the_same_day() {
  let (_app, logs) = logs();
  let first = DailyLog::open(&logs, PREFIX, sep_27(9)).expect("open the log");
  first.write_at(sep_27(9), b"first start\n").expect("write");
  drop(first);
  let second = DailyLog::open(&logs, PREFIX, sep_27(15)).expect("open the log again");
  second.write_at(sep_27(15), b"second start\n").expect("write");

  assert_eq!(read(&log_path(&logs, sep_27(9))), "first start\nsecond start\n");
}

#[test]
fn daily_log_begins_the_next_days_file_at_midnight() {
  let (_app, logs) = logs();
  let log = DailyLog::open(&logs, PREFIX, sep_27(23)).expect("open the log");
  log.write_at(sep_27(23), b"before midnight\n").expect("write");
  log.write_at(sep_27(24), b"after midnight\n").expect("write");

  assert_eq!(read(&logs.join("orchester-2026-09-27.log")), "before midnight\n");
  assert_eq!(read(&logs.join("orchester-2026-09-28.log")), "after midnight\n");
}

#[test]
fn daily_log_keeps_the_last_days_and_deletes_only_older_log_files() {
  let (_app, logs) = logs();
  let now = sep_27(9);
  fs::create_dir_all(&logs).expect("create the log folder");
  let oldest_kept = log_path(&logs, now - DAY * (KEPT_DAYS - 1));
  let too_old = log_path(&logs, now - DAY * KEPT_DAYS);
  let much_older = log_path(&logs, now - DAY * 400);
  let not_logs = [
    "notes.txt",
    "orchester-latest.log",
    "runner-2020-01-01.log",
    "orchester-app-2020-01-01.log",
  ]
  .map(|name| logs.join(name));
  for file in [&oldest_kept, &too_old, &much_older].into_iter().chain(&not_logs) {
    fs::write(file, "old\n").expect("write a file");
  }

  daily_log::prune(&logs, PREFIX, now).expect("prune");

  assert!(oldest_kept.exists());
  assert!(!too_old.exists());
  assert!(!much_older.exists());
  for file in &not_logs {
    assert!(file.exists(), "{} is not a log file", file.display());
  }
}

#[test]
fn daily_log_new_day_deletes_the_file_no_longer_kept() {
  let (_app, logs) = logs();
  let now = sep_27(23);
  let log = DailyLog::open(&logs, PREFIX, now).expect("open the log");
  let last_kept_today = log_path(&logs, now - DAY * (KEPT_DAYS - 1));
  fs::write(&last_kept_today, "old\n").expect("write an old log");

  log.write_at(now, b"today\n").expect("write");
  assert!(last_kept_today.exists());
  log.write_at(sep_27(24), b"tomorrow\n").expect("write");
  assert!(!last_kept_today.exists());
}
