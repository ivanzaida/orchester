//! JUnit XML: `<testsuites>` or one `<testsuite>`, suites nested in suites, and `<testcase>`s with a `<failure>`,
//! `<error>` or `<skipped>` and their `<system-out>` and `<system-err>`. `file` and `line` are attributes some
//! runners write on a test case.

use roxmltree::{Document, Node};

use super::{
  ReportFormat, TestReportError,
  case::{CaseParts, SourceLocation, TestCase, TestOutcome, seconds},
  error::Position,
  report::{RunError, TestReport, Totals},
};

const FORMAT: ReportFormat = ReportFormat::JunitXml;

pub(super) fn read(text: &str) -> Result<TestReport, TestReportError> {
  let document = Document::parse(text).map_err(|error| unreadable(&error))?;
  let root = document.root_element();
  if !matches!(root.tag_name().name(), "testsuites" | "testsuite") {
    let detail = format!(
      "the root element is <{}>, not <testsuites> or <testsuite>",
      root.tag_name().name()
    );
    return Err(malformed(&document, root, detail, &[]));
  }
  let mut reader = Reader {
    document: &document,
    tests: Vec::new(),
    run_errors: Vec::new(),
  };
  reader.suite(root, &[])?;
  Ok(TestReport::new(FORMAT, reader.tests, reader.run_errors))
}

fn unreadable(error: &roxmltree::Error) -> TestReportError {
  let read = Totals::default();
  if matches!(
    error,
    roxmltree::Error::UnclosedRootNode | roxmltree::Error::UnexpectedEndOfStream
  ) {
    return TestReportError::Truncated {
      format: FORMAT,
      detail: error.to_string(),
      read,
    };
  }
  let pos = error.pos();
  let at = Position {
    line: pos.row as usize,
    column: pos.col as usize,
  };
  TestReportError::Malformed {
    format: FORMAT,
    at,
    detail: error.to_string(),
    read,
  }
}

fn malformed(document: &Document<'_>, node: Node<'_, '_>, detail: String, read: &[TestCase]) -> TestReportError {
  let pos = document.text_pos_at(node.range().start);
  let at = Position {
    line: pos.row as usize,
    column: pos.col as usize,
  };
  TestReportError::Malformed {
    format: FORMAT,
    at,
    detail,
    read: Totals::of(read),
  }
}

struct Reader<'d, 'input> {
  document: &'d Document<'input>,
  tests: Vec<TestCase>,
  run_errors: Vec<RunError>,
}

impl Reader<'_, '_> {
  fn suite(&mut self, node: Node<'_, '_>, path: &[String]) -> Result<(), TestReportError> {
    let mut path = path.to_vec();
    let is_suite = node.has_tag_name("testsuite");
    if let Some(name) = node.attribute("name").filter(|name| is_suite && !name.is_empty()) {
      path.push(name.to_owned());
    }
    let first = self.tests.len();
    for child in node.children().filter(Node::is_element) {
      match child.tag_name().name() {
        "testsuite" => self.suite(child, &path)?,
        "testcase" => {
          let case = self.case(child, &path)?;
          self.tests.push(case);
        }
        _ => {}
      }
    }
    if is_suite {
      self.check_counts(node, &path, first);
    }
    Ok(())
  }

  /// A suite whose own `failures` and `errors` name more than its test cases show failed something outside them
  /// (a setup that failed, a runner that crashed): that must not read as a pass.
  fn check_counts(&mut self, node: Node<'_, '_>, path: &[String], first: usize) {
    let declared = ["failures", "errors"]
      .iter()
      .filter_map(|name| node.attribute(*name)?.trim().parse::<usize>().ok())
      .sum::<usize>();
    let shown = self.tests[first..]
      .iter()
      .filter(|test| test.outcome().is_failure())
      .count();
    if declared > shown {
      let message = format!(
        "the suite {} reports {declared} failures and errors, but its test cases show {shown}",
        path.join(" › ")
      );
      self.run_errors.push(RunError::new(&message, None));
    }
  }

  fn case(&self, node: Node<'_, '_>, path: &[String]) -> Result<TestCase, TestReportError> {
    let Some(name) = node.attribute("name") else {
      return Err(malformed(
        self.document,
        node,
        "a <testcase> has no name".to_owned(),
        &self.tests,
      ));
    };
    let mut suite = path.to_vec();
    if let Some(class) = node.attribute("classname").filter(|class| !class.is_empty())
      && suite.last().is_none_or(|last| last != class)
    {
      suite.push(class.to_owned());
    }
    let mut parts = CaseParts::new(suite, name.to_owned(), TestOutcome::Passed);
    parts.duration = node
      .attribute("time")
      .and_then(|time| time.trim().parse::<f64>().ok())
      .and_then(seconds);
    parts.location = node.attribute("file").map(|file| SourceLocation {
      file: file.to_owned(),
      line: node.attribute("line").and_then(|line| line.trim().parse::<u32>().ok()),
    });
    for child in node.children().filter(Node::is_element) {
      read_child(&mut parts, child);
    }
    Ok(parts.finish())
  }
}

fn read_child(parts: &mut CaseParts, child: Node<'_, '_>) {
  let outcome = match child.tag_name().name() {
    "failure" => TestOutcome::Failed,
    "error" => TestOutcome::Errored,
    "skipped" => TestOutcome::Skipped,
    "system-out" | "system-err" => {
      parts.output.push_str(&text_of(child));
      return;
    }
    _ => return,
  };
  // An error outweighs a failure. A `<skipped>` beside a failure marks an expected failure (node's `todo` tests)
  // and outweighs both.
  let rank = |outcome: TestOutcome| match outcome {
    TestOutcome::Passed => 0,
    TestOutcome::Failed => 1,
    TestOutcome::Errored => 2,
    TestOutcome::Skipped => 3,
  };
  if rank(outcome) <= rank(parts.outcome) {
    return;
  }
  parts.outcome = outcome;
  let attribute = child.attribute("message").unwrap_or_default().to_owned();
  let message = match outcome {
    TestOutcome::Skipped if !attribute.trim().is_empty() => vec![attribute],
    _ => [attribute, text_of(child)]
      .into_iter()
      .filter(|part| !part.trim().is_empty())
      .collect::<Vec<_>>(),
  };
  parts.message = (!message.is_empty()).then(|| message.join("\n"));
}

fn text_of(node: Node<'_, '_>) -> String {
  node
    .descendants()
    .filter(Node::is_text)
    .filter_map(|text| text.text())
    .collect()
}
