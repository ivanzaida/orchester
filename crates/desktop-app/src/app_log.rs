//! The app's log, through `tracing`: `orchester-app-<YYYY-MM-DD>.log` (UTC date) in the application log
//! folder Orchester's apps share (`orchester_store::location::log_dir`: beside the executable, or
//! `~/Library/Logs/Orchester` for a macOS app bundle), kept by the shared daily log
//! (`orchester_ipc::daily_log`). Debug builds also log to the console, and lurq's MCP log layer keeps
//! the recent lines for agents (`lurq_logs`). While the error reports' client runs, each ERROR line is a
//! report too ([`error_reports`]).
//!
//! Without a log file (the folder cannot be created or written) the log goes to the console only.

use std::{panic, path::PathBuf, sync::Arc, time::SystemTime};

use orchester_ipc::daily_log::{self, DailyLog};
use orchester_store::location;
use sentry::integrations::tracing::ErrorLayer;
use tracing::{Level, Subscriber};
use tracing_subscriber::{
  EnvFilter, Layer, filter::filter_fn, fmt, layer::SubscriberExt, registry::LookupSpan, util::SubscriberInitExt,
};

/// What the log keeps unless `RUST_LOG` says otherwise: the app's INFO lines, and only warnings and
/// errors from lurq, which logs its reactivity diagnostics at INFO several times per frame.
pub const DEFAULT_FILTER: &str = "info,lurq=warn";
/// The name of the app's log files before their date, apart from the deprecated desktop's `orchester-`.
pub const LOG_PREFIX: &str = "orchester-app-";
/// The target of the TRACE lines that say a screen started reading project data, each with what it reads
/// (`read`): what a change costs in reads (`RUST_LOG=info,orchester_app::read=trace`).
pub const READ_LOG: &str = "orchester_app::read";
/// The target of the ERROR line a panic writes ([`log_panics`]). It makes no report of its own: the client's
/// panic integration reports the panic already, with its stack trace.
pub const PANIC_LOG: &str = "orchester_app::panic";

/// Sends the log to today's file, to lurq's MCP log layer, to [`error_reports`] and, in debug builds, to
/// the console; panics are logged too, since a release build has no console to print them to. Returns
/// today's log file, which a startup screen offers; `None` without one.
pub fn install() -> Option<PathBuf> {
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
    .with(error_reports())
    .init();
  log_panics();
  if let Some(error) = unreadable {
    tracing::warn!(%error, default = DEFAULT_FILTER, "RUST_LOG is not a log filter; using the default");
  }
  match log {
    Ok(log) => {
      let file = daily_log::log_path(log.dir(), LOG_PREFIX, now);
      tracing::info!(file = %file.display(), "application log");
      if let Err(error) = daily_log::prune(log.dir(), LOG_PREFIX, now) {
        tracing::warn!(%error, "cannot delete old application log files");
      }
      Some(file)
    }
    Err(error) => {
      tracing::warn!(%error, "no application log file; logging to the console only");
      None
    }
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

/// The layer that makes each ERROR line an error report while the client runs (`error_reports::start`), and does
/// nothing without one. WARN and below are neither reports nor breadcrumbs, and the panic's own line
/// ([`PANIC_LOG`]) is left to the panic integration. The line's fields go along for `error_reports::options` to
/// put into the message, which the scrubber then cleans.
pub fn error_reports<S>() -> impl Layer<S>
where
  S: Subscriber + for<'span> LookupSpan<'span>,
{
  ErrorLayer::new().with_filter(filter_fn(|line| {
    *line.level() == Level::ERROR && line.target() != PANIC_LOG
  }))
}

/// Logs each panic at ERROR ([`PANIC_LOG`]) before the hook that was set until now runs.
pub fn log_panics() {
  let default = panic::take_hook();
  panic::set_hook(Box::new(move |info| {
    tracing::error!(target: PANIC_LOG, "{info}");
    default(info);
  }));
}
