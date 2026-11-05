//! What `run_tests` and `test_output` answer an agent from a [`SuiteRun`]: the totals and each failure with its
//! message, file:line, excerpt and artifacts, never the program's raw output; and one test's captured output, capped,
//! when it is really needed.

use std::fmt::Write;

use serde_json::{Value, json};

use super::{
  TestCase, excerpt,
  run::{SuiteRun, SuiteRunEnd, test_name},
};
use crate::orchester_tools::ToolResult;

/// The most failures one answer lists; the rest are counted, and `test_output` or the stored result has them.
pub const MAX_FAILURES_SHOWN: usize = 30;
/// The most characters of a failure's message an answer shows; its excerpt has the start of the rest.
const MESSAGE_CHARS: usize = 400;
/// The most bytes of one test's output `test_output` answers: its end, where a failure shows.
pub const TEST_OUTPUT_BYTES: usize = 48 * 1024;

impl SuiteRun {
  /// The answer of the `run_tests` call that made this run, its result kept at `stored`. A report that could not be
  /// read is an error that says so with the exit code and the program's last lines; a stop at the timeout is one too.
  pub fn answer(&self, stored: &str) -> ToolResult {
    let report = match &self.end {
      SuiteRunEnd::Read { report } => report,
      SuiteRunEnd::Unreadable { error, last_lines } => {
        return ToolResult::unavailable(&format!(
          "the suite `{}` did not pass: its report cannot be read ({error}); exit code {}. Its last lines:\n{last_lines}\n\
           [kept as {} in {stored}]",
          self.suite,
          exit_code(self.exit_code),
          self.id
        ));
      }
      SuiteRunEnd::TimedOut { timeout_secs } => {
        return ToolResult::unavailable(&format!(
          "the suite `{}` did not pass: it did not finish within {timeout_secs} seconds and was stopped with \
           everything it started. [kept as {} in {stored}]",
          self.suite, self.id
        ));
      }
    };
    let failures: Vec<&TestCase> = report.failures().collect();
    let shown: Vec<Value> = failures
      .iter()
      .take(MAX_FAILURES_SHOWN)
      .map(|case| self.failure(case))
      .collect();
    let run_errors: Vec<Value> = report
      .run_errors()
      .iter()
      .map(|error| json!({"message": short(error.message()), "location": error.location().map(located)}))
      .collect();
    // A file a glob found and the report attached to a test is one copy, listed once.
    let mut artifacts: Vec<&str> = Vec::new();
    for artifact in &self.artifacts {
      if !artifacts.contains(&artifact.stored.as_str()) {
        artifacts.push(&artifact.stored);
      }
    }
    let mut answer = json!({
      "run": self.id.to_string(),
      "suite": self.suite,
      "filter": self.filter,
      "passed": self.passed(),
      "exit_code": self.exit_code,
      "totals": report.totals(),
      "failures": shown,
      "run_errors": run_errors,
      "artifacts": artifacts,
      "result": stored,
    });
    if failures.len() > MAX_FAILURES_SHOWN {
      answer["more_failures"] = json!(failures.len() - MAX_FAILURES_SHOWN);
    }
    if !self.artifact_problems.is_empty() {
      answer["artifact_problems"] = json!(self.artifact_problems);
    }
    if let Some(note) = self.note() {
      answer["note"] = json!(note);
    }
    ToolResult::shown(answer.to_string())
  }

  fn failure(&self, case: &TestCase) -> Value {
    let name = test_name(case);
    let artifacts: Vec<&str> = self.artifacts_of(&name).collect();
    json!({
      "name": name,
      "outcome": case.outcome(),
      "message": case.message().map(short),
      "location": case.location().map(located),
      "excerpt": case.excerpt(),
      "artifacts": artifacts,
    })
  }

  /// What the totals alone do not say: an exit code that fails a report without failures, or a run of no test.
  fn note(&self) -> Option<String> {
    let report = self.report()?;
    if report.passed() && self.exit_code != Some(0) {
      return Some(format!(
        "the report shows no failure, but the program exited with {}, so the suite did not pass",
        exit_code(self.exit_code)
      ));
    }
    (report.tests().is_empty()).then(|| "no test ran; the filter may match none".to_owned())
  }

  /// The `test_output` answer for the test named `name`: its outcome, message and captured output, the end of it
  /// within [`TEST_OUTPUT_BYTES`].
  pub fn test_output(&self, name: &str) -> ToolResult {
    let Some(report) = self.report() else {
      return ToolResult::unavailable(&format!(
        "the test run {} has no report, so it has no test's output",
        self.id
      ));
    };
    let exact: Vec<&TestCase> = report.tests().iter().filter(|case| test_name(case) == name).collect();
    let found: Vec<&TestCase> = if exact.is_empty() {
      report.tests().iter().filter(|case| case.name() == name).collect()
    } else {
      exact
    };
    let case = match found.as_slice() {
      [case] => *case,
      [] => {
        return ToolResult::unavailable(&format!(
          "the test run {} has no test named `{name}`; use a name run_tests returned",
          self.id
        ));
      }
      several => {
        let names: Vec<String> = several.iter().map(|case| format!("`{}`", test_name(case))).collect();
        return ToolResult::unavailable(&format!(
          "`{name}` names several tests of the test run {}: {}",
          self.id,
          names.join(", ")
        ));
      }
    };
    ToolResult::shown(output_text(case))
  }
}

fn output_text(case: &TestCase) -> String {
  let mut text = format!("{}: {}", test_name(case), case.outcome().as_str());
  // Writing to a String cannot fail.
  if let Some(location) = case.location() {
    let _ = write!(text, " at {}", located(location));
  }
  if let Some(message) = case.message() {
    let _ = write!(text, "\nmessage:\n{message}");
  }
  match case.output() {
    None => text.push_str("\noutput: none captured"),
    Some(output) => {
      let (kept, cut) = excerpt::tail(output.text(), TEST_OUTPUT_BYTES);
      let left_out = output.omitted_bytes() + cut;
      text.push_str("\noutput:\n");
      if left_out > 0 {
        let _ = writeln!(text, "[{left_out} bytes at the start left out]");
      }
      text.push_str(kept);
    }
  }
  text
}

fn located(location: &super::SourceLocation) -> String {
  match location.line {
    Some(line) => format!("{}:{line}", location.file),
    None => location.file.clone(),
  }
}

/// The start of `message`, its first [`MESSAGE_CHARS`] characters.
fn short(message: &str) -> String {
  match message.char_indices().nth(MESSAGE_CHARS) {
    Some((end, _)) => format!("{}…", &message[..end]),
    None => message.to_owned(),
  }
}

fn exit_code(code: Option<i32>) -> String {
  code.map_or_else(|| "none (stopped or ended by a signal)".to_owned(), |code| code.to_string())
}
