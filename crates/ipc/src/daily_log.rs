//! An application log kept in one file per UTC day, `<dir>/<prefix><YYYY-MM-DD>.log`, shared by the
//! desktop apps (each with its own prefix, in the folder `orchester_store::location` gives them). A
//! session that runs past midnight continues in the next day's file. The files of the last
//! [`KEPT_DAYS`] days are kept; older ones of the same prefix are deleted when the log opens a new day.

use std::{
  fs::{self, File, OpenOptions},
  io::{self, Write},
  path::{Path, PathBuf},
  time::{Duration, SystemTime},
};

use parking_lot::Mutex;

use crate::utc_date;

/// Days of logs kept, today included.
pub const KEPT_DAYS: u32 = 14;
const EXTENSION: &str = ".log";
const DAY: Duration = Duration::from_secs(86_400);

/// The log file in `dir` named with `prefix` for the UTC day of `at`.
pub fn log_path(dir: &Path, prefix: &str, at: SystemTime) -> PathBuf {
  file_of(dir, prefix, &utc_date(at))
}

fn file_of(dir: &Path, prefix: &str, date: &str) -> PathBuf {
  dir.join(format!("{prefix}{date}{EXTENSION}"))
}

/// Appends each event to the file of the day it happens on. `&DailyLog` is the writer, so an
/// `Arc<DailyLog>` is what a log subscriber writes through.
pub struct DailyLog {
  dir: PathBuf,
  prefix: &'static str,
  day: Mutex<Day>,
}

/// The open file of one UTC day.
struct Day {
  date: String,
  file: File,
}

impl Day {
  fn open(dir: &Path, prefix: &str, date: String) -> io::Result<Self> {
    let file = OpenOptions::new()
      .create(true)
      .append(true)
      .open(file_of(dir, prefix, &date))?;
    Ok(Self { date, file })
  }
}

impl DailyLog {
  /// Opens the file of `now`'s day in `dir`, creating the folder.
  pub fn open(dir: &Path, prefix: &'static str, now: SystemTime) -> io::Result<Self> {
    fs::create_dir_all(dir)?;
    let day = Day::open(dir, prefix, utc_date(now))?;
    Ok(Self {
      dir: dir.to_path_buf(),
      prefix,
      day: Mutex::new(day),
    })
  }

  pub fn dir(&self) -> &Path {
    &self.dir
  }

  /// Appends `bytes` to the file of `now`'s day. The first write of a day starts its file and deletes
  /// the files no longer kept.
  pub fn write_at(&self, now: SystemTime, bytes: &[u8]) -> io::Result<()> {
    // Before locking: nothing may panic while the lock is held, because a panic hook may log through
    // this writer.
    let date = utc_date(now);
    let mut day = self.day.lock();
    if day.date != date {
      *day = Day::open(&self.dir, self.prefix, date)?;
      // Logging the failure here would write into this log again; a file that cannot be deleted now
      // is deleted, or reported, when the app next starts.
      drop(prune(&self.dir, self.prefix, now));
    }
    day.file.write_all(bytes)
  }
}

impl Write for &DailyLog {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.write_at(SystemTime::now(), bytes)?;
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    self.day.lock().file.flush()
  }
}

/// Deletes the log files named with `prefix` in `dir` from before the last [`KEPT_DAYS`] days up to
/// `now`. Files not named like such a log file are never touched. Every old file is tried; the first
/// failure is returned.
pub fn prune(dir: &Path, prefix: &str, now: SystemTime) -> io::Result<()> {
  let Some(oldest_kept) = now
    .checked_sub(DAY * (KEPT_DAYS - 1))
    .filter(|oldest| *oldest >= SystemTime::UNIX_EPOCH)
  else {
    return Ok(());
  };
  let oldest_kept = utc_date(oldest_kept);
  let mut failure = None;
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let name = entry.file_name();
    let old = name
      .to_str()
      .and_then(|name| log_date(name, prefix))
      .is_some_and(|date| date < oldest_kept.as_str());
    if old && let Err(error) = fs::remove_file(entry.path()) {
      failure.get_or_insert(error);
    }
  }
  failure.map_or(Ok(()), Err)
}

/// The date of a log file named `<prefix>YYYY-MM-DD.log`.
fn log_date<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
  let date = name.strip_prefix(prefix)?.strip_suffix(EXTENSION)?;
  let is_date = date.len() == 10
    && date.bytes().enumerate().all(|(index, byte)| match index {
      4 | 7 => byte == b'-',
      _ => byte.is_ascii_digit(),
    });
  is_date.then_some(date)
}
