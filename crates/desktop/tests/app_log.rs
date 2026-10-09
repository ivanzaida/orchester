//! The desktop's log keeps Orchester's INFO lines and only lurq's warnings and errors by default. The
//! daily files themselves are `orchester_ipc::daily_log`'s, tested there.

use std::{fs, sync::Arc, time::SystemTime};

use orchester_desktop::app_log::{DEFAULT_FILTER, LOG_PREFIX};
use orchester_ipc::daily_log::{self, DailyLog};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

#[test]
fn app_log_default_filter_keeps_orchesters_info_and_drops_lurqs_reactivity_lines() {
  let app = tempfile::tempdir().expect("temporary directory");
  let logs = app.path().join("logs");
  let now = SystemTime::now();
  let log = Arc::new(DailyLog::open(&logs, LOG_PREFIX, now).expect("open the log"));
  let subscriber = tracing_subscriber::registry()
    .with(EnvFilter::new(DEFAULT_FILTER))
    .with(fmt::layer().with_ansi(false).with_writer(Arc::clone(&log)));
  tracing::subscriber::with_default(subscriber, || {
    tracing::info!(target: "lurq::reactivity", "[lurq:reactivity] future_poll completed=true");
    tracing::warn!(target: "lurq::vlist", "items changed in place");
    tracing::info!(target: "orchester_desktop::close", "closing the application");
    tracing::info!(target: "orchester_runner", "run executing");
  });

  let written = fs::read_to_string(daily_log::log_path(&logs, LOG_PREFIX, now)).expect("the log file");
  assert!(!written.contains("future_poll"), "{written}");
  for line in ["items changed in place", "closing the application", "run executing"] {
    assert!(written.contains(line), "{line} in {written}");
  }
}
