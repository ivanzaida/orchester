//! The desktop's own log: `logs/orchester-<YYYY-MM-DD>.log` (UTC date, like the runner logs) in the
//! application log folder ([`app_paths::log_dir`]), kept by the shared daily log
//! (`orchester_ipc::daily_log`, which keeps [`KEPT_DAYS`] days).
//!
//! Without a log file (the folder cannot be created or written) the log goes to the console only.

use std::{panic, sync::Arc, time::SystemTime};

pub use orchester_ipc::daily_log::KEPT_DAYS;
use orchester_ipc::daily_log::{self, DailyLog};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::app_paths;

/// What the log keeps unless `RUST_LOG` says otherwise: Orchester's INFO lines, and only warnings
/// and errors from lurq, which logs its reactivity diagnostics at INFO several times per frame.
pub const DEFAULT_FILTER: &str = "info,lurq=warn";
/// The name of the desktop's log files before their date.
pub const LOG_PREFIX: &str = "orchester-";

/// Sends the desktop's log to today's file and, in debug builds, to the console as well;
/// panics are logged too, since a release build has no console to print them to.
pub fn install() {
  let now = SystemTime::now();
  let log = app_paths::log_dir().and_then(|dir| DailyLog::open(&dir, LOG_PREFIX, now).map(Arc::new));
  let file = log
    .as_ref()
    .ok()
    .map(|log| fmt::layer().with_ansi(false).with_writer(Arc::clone(log)));
  let console = (cfg!(debug_assertions) || log.is_err()).then(fmt::layer);
  tracing_subscriber::registry()
    .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER)))
    .with(console)
    .with(file)
    .init();
  log_panics();
  match log {
    Ok(log) => {
      tracing::info!(file = %daily_log::log_path(log.dir(), LOG_PREFIX, now).display(), "application log");
      if let Err(error) = daily_log::prune(log.dir(), LOG_PREFIX, now) {
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
