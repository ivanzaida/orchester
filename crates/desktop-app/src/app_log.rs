//! The app's log, through `tracing`: `orchester-app-<YYYY-MM-DD>.log` (UTC date) in the application log
//! folder Orchester's apps share (`orchester_store::location::log_dir`: beside the executable, or
//! `~/Library/Logs/Orchester` for a macOS app bundle), kept by the shared daily log
//! (`orchester_ipc::daily_log`). Debug builds also log to the console, and lurq's MCP log layer keeps
//! the recent lines for agents (`lurq_logs`).
//!
//! Without a log file (the folder cannot be created or written) the log goes to the console only.

use std::{panic, sync::Arc, time::SystemTime};

use orchester_ipc::daily_log::{self, DailyLog};
use orchester_store::location;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// What the log keeps unless `RUST_LOG` says otherwise: the app's INFO lines, and only warnings and
/// errors from lurq, which logs its reactivity diagnostics at INFO several times per frame.
pub const DEFAULT_FILTER: &str = "info,lurq=warn";
/// The name of the app's log files before their date, apart from the deprecated desktop's `orchester-`.
pub const LOG_PREFIX: &str = "orchester-app-";

/// Sends the log to today's file, to lurq's MCP log layer and, in debug builds, to the console; panics
/// are logged too, since a release build has no console to print them to.
pub fn install() {
  let now = SystemTime::now();
  let log = location::log_dir().and_then(|dir| DailyLog::open(&dir, LOG_PREFIX, now).map(Arc::new));
  let file = log
    .as_ref()
    .ok()
    .map(|log| fmt::layer().with_ansi(false).with_writer(Arc::clone(log)));
  let console = (cfg!(debug_assertions) || log.is_err()).then(fmt::layer);
  let (filter, unreadable) = filter();
  tracing_subscriber::registry()
    .with(filter)
    .with(console)
    .with(file)
    .with(lurq::mcp::log_layer())
    .init();
  log_panics();
  if let Some(error) = unreadable {
    tracing::warn!(%error, default = DEFAULT_FILTER, "RUST_LOG is not a log filter; using the default");
  }
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

/// The filter `RUST_LOG` sets, or the default with the reason `RUST_LOG` could not be used when it is
/// set (it is logged once the log runs).
fn filter() -> (EnvFilter, Option<String>) {
  match EnvFilter::try_from_default_env() {
    Ok(filter) => (filter, None),
    Err(error) => {
      let set = std::env::var_os(EnvFilter::DEFAULT_ENV).is_some();
      (EnvFilter::new(DEFAULT_FILTER), set.then(|| error.to_string()))
    }
  }
}

fn log_panics() {
  let default = panic::take_hook();
  panic::set_hook(Box::new(move |info| {
    tracing::error!("{info}");
    default(info);
  }));
}
