//! libtest's JSON lines (`cargo test -- -Z unstable-options --format json --report-time`): one suite per test
//! binary, each test's `started` and then its result. A failed test's captured output holds its panic, which gives the
//! message and the file and line.

use serde::Deserialize;

use super::{
  ReportFormat, TestReportError,
  case::{CaseParts, SourceLocation, TestCase, TestOutcome, seconds},
  error::Position,
  report::{TestReport, Totals},
};

const FORMAT: ReportFormat = ReportFormat::CargoJson;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
  Suite {
    event: SuiteEvent,
  },
  Test(TestLine),
  /// Benchmarks and events newer than this reader.
  #[serde(other)]
  Other,
}

#[derive(Deserialize)]
struct TestLine {
  event: TestEvent,
  name: String,
  #[serde(default)]
  exec_time: Option<f64>,
  #[serde(default)]
  stdout: Option<String>,
  #[serde(default)]
  message: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum SuiteEvent {
  Started,
  Ok,
  Failed,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum TestEvent {
  Started,
  Ok,
  Failed,
  Ignored,
  /// A warning that the test runs long, not a result.
  Timeout,
}

impl TestEvent {
  const fn outcome(&self) -> Option<TestOutcome> {
    match self {
      Self::Ok => Some(TestOutcome::Passed),
      Self::Failed => Some(TestOutcome::Failed),
      Self::Ignored => Some(TestOutcome::Skipped),
      Self::Started | Self::Timeout => None,
    }
  }
}

pub(super) fn read(text: &str) -> Result<TestReport, TestReportError> {
  let last_line = text
    .lines()
    .enumerate()
    .filter(|(_, line)| !line.trim().is_empty())
    .last()
    .map_or(0, |(index, _)| index);
  let mut reader = Reader::default();
  for (index, line) in text.lines().enumerate() {
    let line = line.trim();
    // Without capture (`--nocapture`) a test's own prints land between the events.
    if !line.starts_with('{') {
      continue;
    }
    match serde_json::from_str::<Event>(line) {
      Ok(event) => reader.take(event)?,
      Err(error) if error.is_eof() && index == last_line => {
        return Err(reader.truncated(format!("the last line is cut off: {error}")));
      }
      Err(error) => {
        let at = Position {
          line: index + 1,
          column: error.column(),
        };
        return Err(TestReportError::Malformed {
          format: FORMAT,
          at,
          detail: error.to_string(),
          read: reader.read(),
        });
      }
    }
  }
  reader.finish()
}

#[derive(Default)]
struct Reader {
  tests: Vec<TestCase>,
  running: Vec<String>,
  in_suite: bool,
  seen_events: bool,
}

impl Reader {
  fn take(&mut self, event: Event) -> Result<(), TestReportError> {
    self.seen_events = true;
    match event {
      Event::Suite {
        event: SuiteEvent::Started,
      } => {
        if self.in_suite {
          return Err(self.truncated(self.unfinished("a test binary stopped before its suite ended")));
        }
        self.in_suite = true;
      }
      Event::Suite {
        event: SuiteEvent::Ok | SuiteEvent::Failed,
      } => self.in_suite = false,
      Event::Test(test) if matches!(test.event, TestEvent::Started) => self.running.push(test.name),
      Event::Test(test) => {
        if let Some(outcome) = test.event.outcome() {
          self.running.retain(|running| *running != test.name);
          self.tests.push(test.into_case(outcome));
        }
      }
      Event::Other => {}
    }
    Ok(())
  }

  fn finish(self) -> Result<TestReport, TestReportError> {
    if !self.seen_events {
      let detail = "no libtest JSON events".to_owned();
      return Err(TestReportError::Malformed {
        format: FORMAT,
        at: Position::line(1),
        detail,
        read: self.read(),
      });
    }
    if self.in_suite || !self.running.is_empty() {
      return Err(self.truncated(self.unfinished("the suite never ended")));
    }
    Ok(TestReport::new(FORMAT, self.tests, Vec::new()))
  }

  fn unfinished(&self, why: &str) -> String {
    match self.running.as_slice() {
      [] => why.to_owned(),
      running => format!("{why}; started but never finished: {}", running.join(", ")),
    }
  }

  fn read(&self) -> Totals {
    Totals::of(&self.tests)
  }

  fn truncated(&self, detail: String) -> TestReportError {
    TestReportError::Truncated {
      format: FORMAT,
      detail,
      read: self.read(),
    }
  }
}

impl TestLine {
  fn into_case(self, outcome: TestOutcome) -> TestCase {
    let (suite, leaf) = match self.name.rsplit_once("::") {
      Some((path, leaf)) => (path.split("::").map(str::to_owned).collect(), leaf.to_owned()),
      None => (Vec::new(), self.name),
    };
    let mut parts = CaseParts::new(suite, leaf, outcome);
    parts.duration = self.exec_time.and_then(seconds);
    let output = self.stdout.unwrap_or_default();
    match Panic::find(&output) {
      Some(panic) => {
        parts.location = panic.location;
        parts.message = Some(panic.message);
      }
      // `should_panic` tests that did not panic (`test did not panic as expected at src\lib.rs:30:6`), and the
      // reason of an ignored test.
      None => {
        parts.location = self
          .message
          .as_deref()
          .and_then(|message| SourceLocation::parse(message.rsplit_once(" at ")?.1));
        parts.message = self.message;
      }
    }
    parts.output = output;
    parts.finish()
  }
}

/// The first panic in a test's output.
struct Panic {
  location: Option<SourceLocation>,
  message: String,
}

impl Panic {
  const MARK: &str = "panicked at ";

  fn find(output: &str) -> Option<Self> {
    let start = output.find(Self::MARK)? + Self::MARK.len();
    let after = &output[start..];
    let (first, rest) = after.split_once('\n').unwrap_or((after, ""));
    // Before Rust 1.73: `panicked at 'message', src/lib.rs:10:5`.
    if let Some(quoted) = first.strip_prefix('\'') {
      let (message, location) = quoted.rsplit_once("', ")?;
      return Some(Self {
        location: SourceLocation::parse(location),
        message: message.to_owned(),
      });
    }
    let location = SourceLocation::parse(first.trim_end().trim_end_matches(':'));
    let message = rest
      .lines()
      .take_while(|line| !line.starts_with("note: ") && !line.starts_with("stack backtrace:"))
      .collect::<Vec<_>>()
      .join("\n");
    Some(Self { location, message })
  }
}
