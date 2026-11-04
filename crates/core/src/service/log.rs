//! A service's output as Orchester keeps it for `service_logs` and a failed start's error
//! (`docs/architecture/agent-testing.md`, "Agent tools"): its latest lines of standard output and error, numbered from
//! 1 in the order they came, at most [`MAX_LOG_LINES`] lines and [`MAX_LOG_BYTES`] bytes, the oldest dropped first. The
//! values of the service's secret variables are replaced in each line before it is kept, so no answer can show them.

use std::collections::VecDeque;

use crate::command::{SecretVariable, redact_secrets};

/// The most lines a service's log keeps.
pub const MAX_LOG_LINES: usize = 5000;
/// The most bytes of text a service's log keeps.
pub const MAX_LOG_BYTES: usize = 1024 * 1024;
/// The most bytes of one line that are kept; the rest of a longer line is dropped.
pub const MAX_LINE_BYTES: usize = 4096;
/// The lines `service_logs` answers when the call gives no `tail`.
pub const DEFAULT_TAIL: u32 = 100;
/// The most lines one `service_logs` call answers.
pub const MAX_TAIL: u32 = 1000;
/// The most bytes of rows one excerpt holds, the latest kept first: below an answer's limit
/// ([`crate::orchester_tools::MAX_ANSWER_BYTES`]), with room for its first row.
pub const MAX_EXCERPT_BYTES: usize = 48 * 1024;
/// How a secret value is shown in a kept line.
const REDACTED: &str = "<redacted>";

/// Which of the service's streams a line came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogStream {
  Stdout,
  Stderr,
}

impl LogStream {
  const fn label(self) -> &'static str {
    match self {
      Self::Stdout => "out",
      Self::Stderr => "err",
    }
  }
}

/// One kept line, its secret values replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
  number: u64,
  stream: LogStream,
  text: String,
}

impl LogLine {
  /// Its place in the service's output, from 1.
  pub const fn number(&self) -> u64 {
    self.number
  }

  pub const fn stream(&self) -> LogStream {
    self.stream
  }

  pub fn text(&self) -> &str {
    &self.text
  }
}

/// The bounded log of one started service.
#[derive(Debug)]
pub struct ServiceLog {
  lines: VecDeque<LogLine>,
  bytes: usize,
  /// The number of the latest line, kept or dropped.
  last: u64,
  secrets: Vec<SecretVariable>,
}

impl ServiceLog {
  /// An empty log of a service started with `secrets`, whose values its lines never show.
  pub fn new(secrets: Vec<SecretVariable>) -> Self {
    Self {
      lines: VecDeque::new(),
      bytes: 0,
      last: 0,
      secrets,
    }
  }

  /// Keeps `text`, one whole line of `stream` without its line ending.
  pub fn push(&mut self, stream: LogStream, text: &str) {
    let kept = cut(redact_secrets(text, &self.secrets));
    self.keep(stream, kept);
  }

  /// Keeps `head`, the start of a line of `stream` longer than [`MAX_LINE_BYTES`] whose rest was dropped unread. Its
  /// end may be the start of a secret value cut off there, which is replaced too.
  pub fn push_cut(&mut self, stream: LogStream, head: &str) {
    let mut kept = redact_secrets(head, &self.secrets);
    let cut_secret = self
      .secrets
      .iter()
      .map(SecretVariable::expose)
      .filter(|value| !value.is_empty())
      .filter_map(|value| {
        (1..value.len())
          .rev()
          .filter(|end| value.is_char_boundary(*end))
          .find(|end| kept.ends_with(&value[..*end]))
      })
      .max();
    if let Some(length) = cut_secret {
      kept.truncate(kept.len() - length);
      kept.push_str(REDACTED);
    }
    self.keep(stream, cut(kept));
  }

  fn keep(&mut self, stream: LogStream, text: String) {
    self.last += 1;
    self.bytes += text.len();
    self.lines.push_back(LogLine {
      number: self.last,
      stream,
      text,
    });
    while self.lines.len() > MAX_LOG_LINES || self.bytes > MAX_LOG_BYTES {
      let Some(dropped) = self.lines.pop_front() else {
        break;
      };
      self.bytes -= dropped.text.len();
    }
  }

  /// The kept lines, oldest first.
  pub fn lines(&self) -> impl Iterator<Item = &LogLine> {
    self.lines.iter()
  }

  /// The number of the latest line the service wrote, kept or dropped; 0 before the first.
  pub const fn last_number(&self) -> u64 {
    self.last
  }

  /// The last `tail` kept lines after line `since`, within [`MAX_EXCERPT_BYTES`], one per row as
  /// `<number> <out|err>| <text>`, with a first row that says which lines they are and what is not kept any more.
  pub fn excerpt(&self, tail: u32, since: u64) -> String {
    let mut rows = Vec::new();
    let mut bytes = 0;
    for line in self.lines.iter().rev().take_while(|line| line.number > since) {
      let row = format!("\n{} {}| {}", line.number, line.stream.label(), line.text);
      if rows.len() == tail as usize || bytes + row.len() > MAX_EXCERPT_BYTES {
        break;
      }
      bytes += row.len();
      rows.push((line.number, row));
    }
    let (Some((last, _)), Some((first, _))) = (rows.first(), rows.last()) else {
      return match self.last {
        0 => "the service wrote no line yet".to_owned(),
        last => format!("no line after line {since}; the latest is line {last}"),
      };
    };
    let oldest = self.lines.front().map_or(1, LogLine::number);
    let mut text = format!("lines {first} to {last} of {}", self.last);
    if oldest > 1 {
      text.push_str(&format!("; lines before {oldest} are not kept"));
    }
    for (_, row) in rows.iter().rev() {
      text.push_str(row);
    }
    text
  }
}

/// `text` within [`MAX_LINE_BYTES`], at a character boundary.
fn cut(mut text: String) -> String {
  if text.len() > MAX_LINE_BYTES {
    let mut end = MAX_LINE_BYTES;
    while !text.is_char_boundary(end) {
      end -= 1;
    }
    text.truncate(end);
  }
  text
}
