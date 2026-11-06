//! A service that crashed after it was ready: what the agent's notice says, the runner note the conversation keeps
//! (stored as `service_crashed` with plain names and lines), its redaction, and the notice as a text block of its own
//! after a tool's answer.

use std::time::Duration;

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  orchester_tools::ToolResult,
  service::{CRASH_LINES, LogStream, ServiceCrash, ServiceLog},
};
use serde_json::json;

use super::name;

fn crash() -> ServiceCrash {
  let mut log = ServiceLog::new(Vec::new());
  for line in 1..=CRASH_LINES + 5 {
    log.push(LogStream::Stdout, &format!("step {line}"));
  }
  log.push(LogStream::Stderr, "Error: db error: FATAL");
  ServiceCrash::new(
    &name("cloud-api"),
    (Some(1), Duration::from_millis(1400)),
    &log,
    &[name("cloud-web")],
  )
}

#[test]
fn service_crash_carries_its_exit_code_uptime_last_lines_and_what_it_stopped() {
  let crash = crash();
  assert_eq!(crash.service, "cloud-api");
  assert_eq!(crash.exit_code, Some(1));
  assert_eq!(crash.uptime_ms, 1400);
  assert_eq!(crash.stopped, ["cloud-web"]);
  assert_eq!(crash.lines.len(), CRASH_LINES);
  assert_eq!(
    crash.lines.last().map(String::as_str),
    Some("26 err| Error: db error: FATAL")
  );
  let notice = crash.notice();
  let mut lines = notice.lines();
  assert_eq!(
    lines.next(),
    Some(
      "Orchester notice: the service `cloud-api` crashed: it exited with code 1, 1.4 s after it started. Orchester \
       stopped the services that need it: `cloud-web`. Its last lines:"
    )
  );
  assert_eq!(lines.last(), Some("26 err| Error: db error: FATAL"));
}

#[test]
fn service_crash_by_a_signal_with_nothing_stopped_and_no_line() {
  let crash = ServiceCrash::new(
    &name("db"),
    (None, Duration::from_secs(3)),
    &ServiceLog::new(Vec::new()),
    &[],
  );
  assert_eq!(
    crash.notice(),
    "Orchester notice: the service `db` crashed: it was ended by a signal, 3.0 s after it started. It wrote no line."
  );
}

#[test]
fn service_crash_note_is_stored_as_service_crashed_and_redacts_its_lines() {
  let mut event = AgentEvent::RunnerProblem {
    problem: RunnerProblem::ServiceCrashed(crash()),
  };
  let stored = serde_json::to_value(&event).expect("stored");
  assert_eq!(stored["problem"]["problem"], "service_crashed");
  assert_eq!(stored["problem"]["service"], "cloud-api");
  assert_eq!(stored["problem"]["stopped"], json!(["cloud-web"]));
  assert_eq!(serde_json::from_value::<AgentEvent>(stored).expect("reads"), event);
  event.redact(&|text: &str| text.replace("FATAL", "<redacted>"));
  let AgentEvent::RunnerProblem {
    problem: RunnerProblem::ServiceCrashed(redacted),
  } = event
  else {
    panic!("still a crash");
  };
  assert_eq!(
    redacted.lines.last().map(String::as_str),
    Some("26 err| Error: db error: <redacted>")
  );
}

#[test]
fn tool_answer_carries_a_notice_as_a_text_block_after_it() {
  let answer = ToolResult::shown("{\"state\":\"crashed\"}".to_owned()).with_notices(vec![crash().notice()]);
  let mcp = answer.mcp();
  let content = mcp["content"].as_array().expect("content");
  assert_eq!(content.len(), 2);
  assert_eq!(content[0]["text"], "{\"state\":\"crashed\"}");
  assert!(
    content[1]["text"]
      .as_str()
      .is_some_and(|text| text.starts_with("Orchester notice: the service `cloud-api` crashed")),
    "{mcp}"
  );
  let plain = ToolResult::shown("ok".to_owned()).mcp();
  assert_eq!(plain["content"].as_array().map(Vec::len), Some(1));
}
