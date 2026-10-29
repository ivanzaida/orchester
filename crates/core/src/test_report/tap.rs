//! TAP version 13: a plan (`1..N`), test lines (`ok 1 - name`, `not ok 2 - name # SKIP why`), and an indented YAML
//! block after a test with its diagnostics (`message`, `at` or `location`, `duration_ms`). Indented lines outside a
//! YAML block (subtests) and unknown lines are passed over, as TAP says; the top level is the result. A run that
//! does not reach its plan is cut off.

use super::{
  ReportFormat, TestReportError,
  case::{CaseParts, SourceLocation, TestCase, TestOutcome, seconds},
  error::Position,
  report::{TestReport, Totals},
};

const FORMAT: ReportFormat = ReportFormat::Tap;

pub(super) fn read(text: &str) -> Result<TestReport, TestReportError> {
  let mut reader = Reader::default();
  for (index, line) in text.lines().enumerate() {
    reader.line(index + 1, line)?;
  }
  reader.finish()
}

#[derive(Default)]
struct Reader {
  plan: Option<(usize, usize)>,
  tests: Vec<TestCase>,
  pending: Option<CaseParts>,
  yaml: Option<Vec<String>>,
}

impl Reader {
  fn line(&mut self, number: usize, line: &str) -> Result<(), TestReportError> {
    if let Some(yaml) = &mut self.yaml {
      if line.trim() == "..." {
        self.close_yaml();
      } else {
        yaml.push(line.to_owned());
      }
      return Ok(());
    }
    if line.starts_with([' ', '\t']) {
      if line.trim() == "---" && self.pending.is_some() {
        self.yaml = Some(Vec::new());
      }
      return Ok(());
    }
    if let Some(reason) = line.strip_prefix("Bail out!") {
      self.flush();
      return Err(TestReportError::BailedOut {
        reason: reason.trim().to_owned(),
        read: self.read(),
      });
    }
    if let Some(count) = plan_count(line) {
      if self.plan.is_some() {
        return Err(self.malformed(number, "a second plan line".to_owned()));
      }
      self.plan = Some((count, number));
      return Ok(());
    }
    if let Some(parts) = test_point(line, self.tests.len() + usize::from(self.pending.is_some()) + 1) {
      self.flush();
      self.pending = Some(parts);
    }
    Ok(())
  }

  fn finish(mut self) -> Result<TestReport, TestReportError> {
    if self.yaml.is_some() {
      return Err(self.truncated("a YAML block never ended".to_owned()));
    }
    self.flush();
    let Some((planned, plan_line)) = self.plan else {
      if self.tests.is_empty() {
        return Err(self.malformed(1, "no plan line and no test lines".to_owned()));
      }
      return Err(self.truncated("no plan line (1..N)".to_owned()));
    };
    let read = self.tests.len();
    if read < planned {
      return Err(self.truncated(format!("the plan names {planned} tests, {read} were reported")));
    }
    if read > planned {
      return Err(self.malformed(
        plan_line,
        format!("the plan names {planned} tests, {read} were reported"),
      ));
    }
    Ok(TestReport::new(FORMAT, self.tests, Vec::new()))
  }

  fn close_yaml(&mut self) {
    let block = self.yaml.take().unwrap_or_default();
    if let Some(parts) = &mut self.pending {
      Diagnostics::parse(&block).apply(parts);
      parts.output = block.join("\n");
    }
    self.flush();
  }

  fn flush(&mut self) {
    if let Some(parts) = self.pending.take() {
      self.tests.push(parts.finish());
    }
  }

  fn read(&self) -> Totals {
    Totals::of(&self.tests)
  }

  fn malformed(&self, line: usize, detail: String) -> TestReportError {
    TestReportError::Malformed {
      format: FORMAT,
      at: Position::line(line),
      detail,
      read: self.read(),
    }
  }

  fn truncated(&self, detail: String) -> TestReportError {
    TestReportError::Truncated {
      format: FORMAT,
      detail,
      read: self.read(),
    }
  }
}

/// `1..N`, or `1..0 # SKIP why` for a run with no tests.
fn plan_count(line: &str) -> Option<usize> {
  let rest = line.strip_prefix("1..")?;
  let digits = rest.split(|c: char| !c.is_ascii_digit()).next()?;
  digits.parse().ok()
}

/// `ok`/`not ok`, an optional number, an optional `-`, the description and a `# SKIP` or `# TODO` directive.
fn test_point(line: &str, next_number: usize) -> Option<CaseParts> {
  let (ok, rest) = match line.strip_prefix("not ok") {
    Some(rest) => (false, rest),
    None => (true, line.strip_prefix("ok")?),
  };
  if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
    return None;
  }
  let rest = rest
    .trim_start()
    .trim_start_matches(|c: char| c.is_ascii_digit())
    .trim_start();
  let rest = rest.strip_prefix('-').unwrap_or(rest);
  let (description, directive) = split_directive(rest);
  let directive = directive.unwrap_or_default();
  let upper = directive.to_ascii_uppercase();
  let outcome = match (ok, upper.starts_with("SKIP"), upper.starts_with("TODO")) {
    (_, true, _) => TestOutcome::Skipped,
    // A failing `TODO` test is an expected failure.
    (false, _, true) => TestOutcome::Skipped,
    (true, ..) => TestOutcome::Passed,
    (false, ..) => TestOutcome::Failed,
  };
  let name = match description.trim() {
    "" => format!("test {next_number}"),
    description => description.to_owned(),
  };
  let mut parts = CaseParts::new(Vec::new(), name, outcome);
  if outcome == TestOutcome::Skipped {
    // The reason after the four letters of `SKIP` or `TODO`.
    parts.message = directive.get(4..).map(|reason| reason.trim().to_owned());
  }
  Some(parts)
}

/// The description and the directive after the first `#` that is not escaped as `\#`.
fn split_directive(text: &str) -> (&str, Option<&str>) {
  let mut escaped = false;
  for (index, c) in text.char_indices() {
    match c {
      '#' if !escaped => return (&text[..index], Some(text[index + 1..].trim())),
      '\\' => escaped = !escaped,
      _ => escaped = false,
    }
  }
  (text, None)
}

/// What a test's YAML block says, read by its keys: TAP producers write a few flat ones, which needs no YAML parser.
#[derive(Default)]
struct Diagnostics {
  message: Option<String>,
  file: Option<String>,
  line: Option<u32>,
  location: Option<SourceLocation>,
  duration_ms: Option<f64>,
}

impl Diagnostics {
  fn parse(block: &[String]) -> Self {
    let mut diagnostics = Self::default();
    let base = block
      .iter()
      .find(|line| !line.trim().is_empty())
      .map_or(0, |line| indent(line));
    let mut parent = String::new();
    for line in block {
      let body = line.trim();
      if body.is_empty() {
        continue;
      }
      if indent(line) > base {
        diagnostics.nested(&parent, body);
        continue;
      }
      let Some((key, value)) = body.split_once(':') else {
        continue;
      };
      parent = key.trim().to_owned();
      diagnostics.value(&parent, unquote(value.trim()));
    }
    diagnostics
  }

  fn value(&mut self, key: &str, value: &str) {
    match key {
      // node-tap writes `message`, node's test runner `error`. A block scalar (`|`, `|-`, `>`) is read from the
      // lines under it.
      "message" | "error" if value.starts_with(['|', '>']) || value.is_empty() => self.message = Some(String::new()),
      "message" | "error" => self.message = Some(value.to_owned()),
      "at" | "location" => self.location = SourceLocation::parse(value.trim_start_matches('(').trim_end_matches(')')),
      "file" => self.file = Some(value.to_owned()),
      "line" => self.line = value.parse().ok(),
      "duration_ms" => self.duration_ms = value.parse().ok(),
      _ => {}
    }
  }

  fn nested(&mut self, parent: &str, body: &str) {
    match parent {
      "message" | "error" => {
        if let Some(message) = &mut self.message {
          if !message.is_empty() {
            message.push('\n');
          }
          message.push_str(body);
        }
      }
      "at" => match body.split_once(':') {
        Some(("file", value)) => self.file = Some(unquote(value.trim()).to_owned()),
        Some(("line", value)) => self.line = value.trim().parse().ok(),
        _ => {}
      },
      _ => {}
    }
  }

  fn apply(self, parts: &mut CaseParts) {
    // A `SKIP` or `TODO` reason stays the message; the error of an expected failure is in the output.
    if parts.message.is_none() {
      parts.message = self.message.filter(|message| !message.is_empty());
    }
    parts.location = match (self.file, self.location) {
      (Some(file), _) => Some(SourceLocation { file, line: self.line }),
      (None, location) => location,
    };
    parts.duration = self.duration_ms.map(|ms| ms / 1000.0).and_then(seconds);
  }
}

fn indent(line: &str) -> usize {
  line.len() - line.trim_start().len()
}

fn unquote(value: &str) -> &str {
  ["'", "\""]
    .iter()
    .find_map(|quote| value.strip_prefix(quote)?.strip_suffix(quote))
    .unwrap_or(value)
}
