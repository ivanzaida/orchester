//! The desktop's own log: `logs/orchester-<YYYY-MM-DD>.log` (UTC date, like the runner logs)
//! in the application log folder ([`app_paths::log_dir`]). A session that runs past midnight
//! continues in the next day's file. The files of the last [`KEPT_DAYS`] days are kept; older
//! ones are deleted when the app starts and when a new day's file begins.
//!
//! Without a log file (the folder cannot be created or written) the log goes to the console only.

use std::{
  fs::{self, File, OpenOptions},
  io::{self, Write},
  panic,
  path::{Path, PathBuf},
  sync::Arc,
  time::{Duration, SystemTime},
};

use orchester_ipc::utc_date;
use parking_lot::Mutex;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::app_paths;

/// Days of logs kept, today included.
pub const KEPT_DAYS: u32 = 14;
const PREFIX: &str = "orchester-";
const EXTENSION: &str = ".log";
const DAY: Duration = Duration::from_secs(86_400);

/// The log file in `dir` for the UTC day of `at`.
pub fn log_path(dir: &Path, at: SystemTime) -> PathBuf {
  file_of(dir, &utc_date(at))
}

fn file_of(dir: &Path, date: &str) -> PathBuf {
  dir.join(format!("{PREFIX}{date}{EXTENSION}"))
}

/// Appends each event to the file of the day it happens on. `&AppLog` is the writer, so an
/// `Arc<AppLog>` is what the log subscriber writes through.
pub struct AppLog {
  dir: PathBuf,
  day: Mutex<Day>,
}

/// The open file of one UTC day.
struct Day {
  date: String,
  file: File,
}

impl Day {
  fn open(dir: &Path, date: String) -> io::Result<Self> {
    let file = OpenOptions::new().create(true).append(true).open(file_of(dir, &date))?;
    Ok(Self { date, file })
  }
}

impl AppLog {
  /// Opens the file of `now`'s day in `dir`, creating the folder.
  pub fn open(dir: &Path, now: SystemTime) -> io::Result<Self> {
    fs::create_dir_all(dir)?;
    let day = Day::open(dir, utc_date(now))?;
    Ok(Self {
      dir: dir.to_path_buf(),
      day: Mutex::new(day),
    })
  }

  /// Appends `bytes` to the file of `now`'s day. The first write of a day starts its file and
  /// deletes the files no longer kept.
  pub fn write_at(&self, now: SystemTime, bytes: &[u8]) -> io::Result<()> {
    // Before locking: nothing may panic while the lock is held, because the panic hook logs
    // through this writer.
    let date = utc_date(now);
    let mut day = self.day.lock();
    if day.date != date {
      *day = Day::open(&self.dir, date)?;
      // Logging the failure here would write into this log again; a file that cannot be deleted
      // now is deleted, or reported, when the app next starts.
      drop(prune(&self.dir, now));
    }
    day.file.write_all(bytes)
  }
}

impl Write for &AppLog {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.write_at(SystemTime::now(), bytes)?;
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    self.day.lock().file.flush()
  }
}

/// Deletes the log files in `dir` from before the last [`KEPT_DAYS`] days up to `now`. Files
/// not named like a log file are never touched. Every old file is tried; the first failure is
/// returned.
pub fn prune(dir: &Path, now: SystemTime) -> io::Result<()> {
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
      .and_then(log_date)
      .is_some_and(|date| date < oldest_kept.as_str());
    if old && let Err(error) = fs::remove_file(entry.path()) {
      failure.get_or_insert(error);
    }
  }
  failure.map_or(Ok(()), Err)
}

/// The date of a log file named `orchester-YYYY-MM-DD.log`.
fn log_date(name: &str) -> Option<&str> {
  let date = name.strip_prefix(PREFIX)?.strip_suffix(EXTENSION)?;
  let is_date = date.len() == 10
    && date.bytes().enumerate().all(|(index, byte)| match index {
      4 | 7 => byte == b'-',
      _ => byte.is_ascii_digit(),
    });
  is_date.then_some(date)
}

/// Sends the desktop's log to today's file and, in debug builds, to the console as well;
/// panics are logged too, since a release build has no console to print them to.
pub fn install() {
  let now = SystemTime::now();
  let log = app_paths::log_dir().and_then(|dir| AppLog::open(&dir, now).map(Arc::new));
  let file = log
    .as_ref()
    .ok()
    .map(|log| fmt::layer().with_ansi(false).with_writer(Arc::clone(log)));
  let console = (cfg!(debug_assertions) || log.is_err()).then(fmt::layer);
  tracing_subscriber::registry()
    .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
    .with(console)
    .with(file)
    .init();
  log_panics();
  match log {
    Ok(log) => {
      tracing::info!(file = %log_path(&log.dir, now).display(), "application log");
      if let Err(error) = prune(&log.dir, now) {
        tracing::warn!(%error, "cannot delete old application log files");
      }
    }
    Err(error) => tracing::warn!(%error, "no application log file; logging to the console only"),
  }
}

fn log_panics() {
  let default = panic::take_hook();
  panic::set_hook(Box::new(move |info| {
    tracing::error!("{info}");
    default(info);
  }));
}
