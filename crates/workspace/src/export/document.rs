//! The Markdown an export writes, part by part: the title, its facts, a heading for each step, each message under
//! who wrote it and when with the names of its files, and a tool call with its input and a short result. Text a
//! person or an agent wrote goes in as it was written; names, commands and outputs go in code, fenced longer than
//! any run of backticks they hold, so nothing they say becomes Markdown of the file.

use orchester_core::time::Timestamp;

use super::{Words, stamp};
use crate::messages::Message;

/// Lines of a tool call's result the file keeps, from its start.
const RESULT_LINES: usize = 8;
/// Characters of a tool call's result the file keeps at most.
const RESULT_CHARS: usize = 600;

/// The Markdown of one export, written in order.
pub(super) struct Document<'a> {
  text: String,
  words: Words<'a>,
}

impl<'a> Document<'a> {
  /// A file titled `title`, its words in `words`.
  pub(super) fn new(title: &str, words: Words<'a>) -> Self {
    Self {
      text: format!("# {}\n\n", one_line(title)),
      words,
    }
  }

  /// What `message` says in the person's language.
  pub(super) fn say(&self, message: &Message) -> String {
    (self.words)(message)
  }

  /// One fact under the title: `label` (a key) and its `value`.
  pub(super) fn fact(&mut self, label: &'static str, value: &str) {
    let label = self.say(&Message::new(label));
    let line = self.say(
      &Message::new("export.markdown.fact")
        .with("label", label)
        .with("value", one_line(value)),
    );
    self.text.push_str(&format!("- {line}\n"));
  }

  /// The rule between the facts and the conversation.
  pub(super) fn rule(&mut self) {
    self.text.push_str("\n---\n\n");
  }

  /// A step's heading.
  pub(super) fn step(&mut self, heading: &str) {
    self.text.push_str(&format!("## {}\n\n", one_line(heading)));
  }

  /// A message `body` that `author` wrote at `at`, with the names of the files attached to it.
  pub(super) fn message(&mut self, author: &str, at: Timestamp, body: &str, attachments: &[&str]) {
    self
      .text
      .push_str(&format!("### {} · {}\n\n", one_line(author), stamp(at)));
    let body = body.trim();
    if !body.is_empty() {
      self.text.push_str(body);
      self.text.push_str("\n\n");
    }
    if attachments.is_empty() {
      return;
    }
    let names: Vec<String> = attachments.iter().map(|name| code_span(name)).collect();
    let line = self.say(&Message::new("export.markdown.attachments").with("names", names.join(", ")));
    self.text.push_str(&format!("_{line}_\n\n"));
  }

  /// A tool call: what it is (`what`, a sentence of the file's words), its `input` and a short `result`.
  pub(super) fn tool(&mut self, what: &str, input: &str, result: &str) {
    let input = input.trim();
    if input.contains('\n') {
      self
        .text
        .push_str(&format!("- {}\n\n{}\n", one_line(what), fenced(input, "  ")));
    } else if input.is_empty() {
      self.text.push_str(&format!("- {}\n", one_line(what)));
    } else {
      self
        .text
        .push_str(&format!("- {} {}\n", one_line(what), code_span(input)));
    }
    let result = short(result);
    if !result.is_empty() {
      self.text.push_str(&format!("\n{}\n", fenced(&result, "  ")));
    }
  }

  /// The end of a run of tool calls, before the next message or step.
  pub(super) fn end_tools(&mut self) {
    if !self.text.ends_with("\n\n") {
      self.text.push('\n');
    }
  }

  pub(super) fn finish(mut self) -> String {
    let kept = self.text.trim_end().len();
    self.text.truncate(kept);
    self.text.push('\n');
    self.text
  }
}

/// `text` on one line: its lines joined with spaces.
fn one_line(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first lines of `result`, cut to its share of the file.
fn short(result: &str) -> String {
  let lines: Vec<&str> = result.trim().lines().take(RESULT_LINES).collect();
  let mut kept = lines.join("\n");
  let cut = result.trim().lines().count() > RESULT_LINES || kept.chars().count() > RESULT_CHARS;
  if kept.chars().count() > RESULT_CHARS {
    kept = kept.chars().take(RESULT_CHARS).collect();
  }
  if cut {
    kept.push_str("\n…");
  }
  kept
}

/// The longest run of backticks in `text`.
fn longest_ticks(text: &str) -> usize {
  text.split(|c| c != '`').map(str::len).max().unwrap_or_default()
}

/// `text` as inline code, between more backticks than it holds in a row.
fn code_span(text: &str) -> String {
  let ticks = "`".repeat(longest_ticks(text) + 1);
  let pad = if text.starts_with('`') || text.ends_with('`') {
    " "
  } else {
    ""
  };
  format!("{ticks}{pad}{text}{pad}{ticks}")
}

/// `text` as a fenced block indented by `indent`, its fence longer than any run of backticks it holds.
fn fenced(text: &str, indent: &str) -> String {
  let fence = "`".repeat(longest_ticks(text).max(2) + 1);
  let mut block = format!("{indent}{fence}\n");
  for line in text.lines() {
    block.push_str(indent);
    block.push_str(line);
    block.push('\n');
  }
  block.push_str(indent);
  block.push_str(&fence);
  block.push('\n');
  block
}
