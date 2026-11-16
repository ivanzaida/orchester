//! What `service_status` answers: starting, ready, crashed with its exit code, or stopped, with its port and URL, and
//! for how long it has run while it runs.

use std::time::Duration;

use orchester_core::service::{ServicePhase, ServiceState, ServiceStatus};
use serde_json::{Value, json};

fn answer(phase: ServicePhase, exited: Option<Option<i32>>) -> Value {
  let status = ServiceStatus {
    state: ServiceState::of(phase, exited),
    port: 41234,
    url: "http://127.0.0.1:41234".to_owned(),
    uptime: Duration::from_secs(12),
  };
  serde_json::from_str(&status.answer()).expect("JSON")
}

#[test]
fn service_status_reports_each_state() {
  assert_eq!(
    answer(ServicePhase::Starting, None),
    json!({"state": "starting", "port": 41234, "url": "http://127.0.0.1:41234", "uptime_secs": 12})
  );
  assert_eq!(
    answer(ServicePhase::Ready, None),
    json!({"state": "ready", "port": 41234, "url": "http://127.0.0.1:41234", "uptime_secs": 12})
  );
  assert_eq!(
    answer(ServicePhase::Ready, Some(Some(3))),
    json!({"state": "crashed", "exit_code": 3, "port": 41234, "url": "http://127.0.0.1:41234"})
  );
  assert_eq!(
    answer(ServicePhase::Stopped, None),
    json!({"state": "stopped", "port": 41234, "url": "http://127.0.0.1:41234"})
  );
}

#[test]
fn service_status_keeps_a_crash_once_seen() {
  assert_eq!(
    ServiceState::of(ServicePhase::Stopped, Some(Some(1))),
    ServiceState::Crashed { code: Some(1) },
    "a crashed service stopped later is still crashed"
  );
  assert_eq!(
    ServiceState::of(ServicePhase::Starting, Some(None)),
    ServiceState::Crashed { code: None },
    "a signal ended it before it was ready"
  );
  assert_eq!(answer(ServicePhase::Ready, Some(None))["exit_code"], Value::Null);
}
