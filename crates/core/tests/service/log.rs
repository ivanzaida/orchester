//! A service's log: bounded by lines and bytes, a long line cut, the values of its secret variables never kept (a
//! canary secret), and the excerpts `service_logs` and a failed start answer.

use std::sync::Arc;

use orchester_core::{
  command::SecretVariable,
  secret::SecretValue,
  service::{LogStream, MAX_EXCERPT_BYTES, MAX_LINE_BYTES, MAX_LOG_BYTES, MAX_LOG_LINES, ServiceLog},
};

const CANARY: &str = "canary-7f3a-9c21-secret";

fn canary_log() -> ServiceLog {
  let value = SecretValue::new(CANARY.to_owned()).expect("a value");
  ServiceLog::new(vec![SecretVariable::new("API_TOKEN", Arc::new(value))])
}

fn kept(log: &ServiceLog) -> Vec<String> {
  log.lines().map(|line| line.text().to_owned()).collect()
}

#[test]
fn service_log_keeps_the_latest_lines_within_its_bounds() {
  let mut log = ServiceLog::new(Vec::new());
  for number in 1..=MAX_LOG_LINES + 10 {
    log.push(LogStream::Stdout, &format!("line {number}"));
  }
  assert_eq!(log.lines().count(), MAX_LOG_LINES);
  assert_eq!(
    log.lines().next().expect("a line").number(),
    11,
    "the oldest are dropped"
  );
  assert_eq!(log.last_number(), u64::try_from(MAX_LOG_LINES + 10).expect("fits"));

  let mut wide = ServiceLog::new(Vec::new());
  let long = "x".repeat(MAX_LINE_BYTES * 3);
  for _ in 0..(MAX_LOG_BYTES / MAX_LINE_BYTES) + 20 {
    wide.push(LogStream::Stderr, &long);
  }
  let bytes: usize = wide.lines().map(|line| line.text().len()).sum();
  assert!(bytes <= MAX_LOG_BYTES, "{bytes} bytes kept");
  assert!(
    wide.lines().all(|line| line.text().len() == MAX_LINE_BYTES),
    "a long line is cut"
  );
  assert!(wide.lines().all(|line| line.stream() == LogStream::Stderr));
}

#[test]
fn service_log_never_keeps_a_secret_value() {
  let mut log = canary_log();
  log.push(LogStream::Stdout, &format!("token={CANARY}"));
  log.push(LogStream::Stderr, &format!(r#"{{"token":"{CANARY}"}}"#));
  // A line cut where the secret begins: its start is not kept either.
  let head = format!("{}{}", "y".repeat(40), &CANARY[..9]);
  log.push_cut(LogStream::Stdout, &head);
  let lines = kept(&log);
  assert!(lines.iter().all(|line| !line.contains(CANARY)), "{lines:?}");
  assert!(lines.iter().all(|line| !line.contains(&CANARY[..9])), "{lines:?}");
  assert_eq!(lines[0], "token=<redacted>");
  assert_eq!(lines[2], format!("{}<redacted>", "y".repeat(40)));
  assert!(!log.excerpt(100, 0).contains(CANARY));
}

#[test]
fn service_log_excerpt_answers_the_tail_after_a_line() {
  let mut log = ServiceLog::new(Vec::new());
  assert_eq!(log.excerpt(10, 0), "the service wrote no line yet");
  for number in 1..=5 {
    log.push(LogStream::Stdout, &format!("line {number}"));
  }
  log.push(LogStream::Stderr, "warning");
  assert_eq!(log.excerpt(2, 0), "lines 5 to 6 of 6\n5 out| line 5\n6 err| warning");
  assert_eq!(log.excerpt(100, 4), "lines 5 to 6 of 6\n5 out| line 5\n6 err| warning");
  assert_eq!(log.excerpt(100, 6), "no line after line 6; the latest is line 6");

  let mut long = ServiceLog::new(Vec::new());
  for number in 1..=MAX_LOG_LINES + 1 {
    long.push(LogStream::Stdout, &format!("line {number}"));
  }
  assert!(long.excerpt(1, 0).contains("lines before 2 are not kept"));
}

#[test]
fn service_log_excerpt_stays_within_its_size() {
  let mut log = ServiceLog::new(Vec::new());
  for _ in 0..100 {
    log.push(LogStream::Stdout, &"z".repeat(MAX_LINE_BYTES));
  }
  let excerpt = log.excerpt(1000, 0);
  assert!(excerpt.len() <= MAX_EXCERPT_BYTES + 200, "{} bytes", excerpt.len());
  assert!(excerpt.starts_with("lines "), "the first row says which lines");
  assert!(excerpt.contains(" to 100 of 100"), "the latest lines are kept");
}
