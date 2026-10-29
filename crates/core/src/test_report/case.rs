//! One test of a report, in the shape every format is read into.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::excerpt;

/// The most bytes of a test's captured output kept: its end, where a failure shows.
pub const OUTPUT_CAP_BYTES: usize = 64 * 1024;
/// The most bytes of a test's message kept: its start.
pub const MESSAGE_CAP_BYTES: usize = 16 * 1024;
/// The most lines of a failure's excerpt.
pub const EXCERPT_LINES: usize = 20;
/// The most characters of one excerpt line, so a single huge line cannot fill it.
pub const EXCERPT_LINE_CHARS: usize = 240;

/// How a test ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestOutcome {
  Passed,
  /// An assertion or expectation failed.
  Failed,
  /// Not run, or an expected failure (TAP's `TODO`).
  Skipped,
  /// The test could not run to an answer: an error outside its assertions, or the run was interrupted.
  Errored,
}

impl TestOutcome {
  pub const fn is_failure(self) -> bool {
    matches!(self, Self::Failed | Self::Errored)
  }
}

/// Where a test or its failure is in the source, as the report gives it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
  pub file: String,
  pub line: Option<u32>,
}

impl SourceLocation {
  /// Reads `file:line` or `file:line:column` (`src/lib.rs:10:5`, `C:\repo\a.rs:3`).
  pub(super) fn parse(text: &str) -> Option<Self> {
    let (rest, last) = text.trim().rsplit_once(':')?;
    let last = last.parse::<u32>().ok()?;
    let (file, line) = match rest.rsplit_once(':').map(|(file, line)| (file, line.parse::<u32>())) {
      Some((file, Ok(line))) => (file, line),
      _ => (rest, last),
    };
    (!file.is_empty()).then(|| Self {
      file: file.to_owned(),
      line: Some(line),
    })
  }
}

/// A file a test run left for a person or agent to look at (a Playwright screenshot or trace).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
  pub name: String,
  pub content_type: String,
  pub path: String,
}

/// What a test printed, capped to its last [`OUTPUT_CAP_BYTES`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedOutput {
  text: String,
  omitted_bytes: usize,
}

impl CapturedOutput {
  fn capped(text: &str) -> Option<Self> {
    let text = excerpt::plain(text);
    if text.trim().is_empty() {
      return None;
    }
    let (kept, omitted_bytes) = excerpt::tail(&text, OUTPUT_CAP_BYTES);
    Some(Self {
      text: kept.to_owned(),
      omitted_bytes,
    })
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  /// Bytes left out before [`Self::text`].
  pub const fn omitted_bytes(&self) -> usize {
    self.omitted_bytes
  }
}

/// One test as a report gives it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCase {
  suite: Vec<String>,
  name: String,
  outcome: TestOutcome,
  duration: Option<Duration>,
  message: Option<String>,
  location: Option<SourceLocation>,
  output: Option<CapturedOutput>,
  attachments: Vec<Attachment>,
  excerpt: Option<String>,
}

impl TestCase {
  /// The suites the test is in, outermost first (modules, files, `describe` blocks, a Playwright project).
  pub fn suite(&self) -> &[String] {
    &self.suite
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub const fn outcome(&self) -> TestOutcome {
    self.outcome
  }

  pub const fn duration(&self) -> Option<Duration> {
    self.duration
  }

  /// The failure's or skip's message, without terminal colours, capped to [`MESSAGE_CAP_BYTES`].
  pub fn message(&self) -> Option<&str> {
    self.message.as_deref()
  }

  pub const fn location(&self) -> Option<&SourceLocation> {
    self.location.as_ref()
  }

  pub const fn output(&self) -> Option<&CapturedOutput> {
    self.output.as_ref()
  }

  pub fn attachments(&self) -> &[Attachment] {
    &self.attachments
  }

  /// For a failure, at most [`EXCERPT_LINES`] lines to show first: the message's start, or the output's end when
  /// there is no message.
  pub fn excerpt(&self) -> Option<&str> {
    self.excerpt.as_deref()
  }

  fn excerpt_of(outcome: TestOutcome, message: Option<&str>, output: Option<&CapturedOutput>) -> Option<String> {
    if !outcome.is_failure() {
      return None;
    }
    if let Some(message) = message {
      return Some(excerpt::lines(message.lines(), EXCERPT_LINES, EXCERPT_LINE_CHARS));
    }
    let lines = output?.text().lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(EXCERPT_LINES);
    Some(excerpt::lines(
      lines[start..].iter().copied(),
      EXCERPT_LINES,
      EXCERPT_LINE_CHARS,
    ))
  }
}

/// A test as a parser reads it, before its text is capped.
pub(super) struct CaseParts {
  pub suite: Vec<String>,
  pub name: String,
  pub outcome: TestOutcome,
  pub duration: Option<Duration>,
  pub message: Option<String>,
  pub location: Option<SourceLocation>,
  pub output: String,
  pub attachments: Vec<Attachment>,
}

impl CaseParts {
  pub fn new(suite: Vec<String>, name: String, outcome: TestOutcome) -> Self {
    Self {
      suite,
      name,
      outcome,
      duration: None,
      message: None,
      location: None,
      output: String::new(),
      attachments: Vec::new(),
    }
  }

  pub fn finish(self) -> TestCase {
    let message = capped_message(self.message.as_deref());
    let output = CapturedOutput::capped(&self.output);
    let excerpt = TestCase::excerpt_of(self.outcome, message.as_deref(), output.as_ref());
    TestCase {
      suite: self.suite,
      name: self.name,
      outcome: self.outcome,
      duration: self.duration,
      message,
      location: self.location,
      output,
      attachments: self.attachments,
      excerpt,
    }
  }
}

/// A message without terminal colours, trimmed and capped to [`MESSAGE_CAP_BYTES`]; none when it is blank.
pub(super) fn capped_message(message: Option<&str>) -> Option<String> {
  let plain = excerpt::plain(message?);
  let trimmed = plain.trim();
  if trimmed.is_empty() {
    return None;
  }
  let (kept, omitted) = excerpt::head(trimmed, MESSAGE_CAP_BYTES);
  Some(if omitted == 0 {
    kept.to_owned()
  } else {
    format!("{kept}…")
  })
}

/// A time in seconds as a report writes it; a negative or impossible number is left unknown rather than guessed.
pub(super) fn seconds(seconds: f64) -> Option<Duration> {
  Duration::try_from_secs_f64(seconds).ok()
}
