//! @-mentions of the project's tasks, runs and files in the Orchestrator's and a task Conversation's composer (ORC-312,
//! ORC-316; `docs/design/mentions.md`, `Mentions · redesign` `z8cUk`). A mention is written into the draft as plain
//! text, `@KON-12` for a task, `@KON-12#9` for its run #9 and ``@`src/lib.rs` `` for a file, its path relative to the
//! repository between backticks (so a path with spaces, or one without a dot or a slash, is told from other words), so
//! a draft stays the plain string both composers keep and a composer draws each mention as a chip. The @ menu offers
//! the project's tasks and runs ([`catalog`]) and its repository's files ([`files`]); a message sent with mentions
//! carries a marked block of what Orchester knows of each after the person's words ([`facts`]), which the
//! conversations leave out of what they show. The /watch pickers, which choose modules and tasks with the same
//! search, are [`watch`]'s.

pub mod catalog;
pub mod facts;
pub mod files;
pub mod watch;

use std::{num::NonZeroU32, ops::Range};

use orchester_core::{project::TaskKey, run::RunNumber};

use crate::files::FilePath;

/// What starts a mention.
const AT: char = '@';
/// What puts a run's number after its task's key.
const RUN_MARK: char = '#';
/// What a file's path is written between.
const PATH_QUOTE: char = '`';

/// What a mention points at: a task by its key, a run by its task's key and its number, or a file of the repository by
/// its path relative to the repository's root (a run's worktree, in a run).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mention {
  Task(TaskKey),
  Run(TaskKey, RunNumber),
  File(FilePath),
}

impl Mention {
  /// The task the mention names, or whose run it names; none for a file.
  pub const fn task(&self) -> Option<&TaskKey> {
    match self {
      Self::Task(key) | Self::Run(key, _) => Some(key),
      Self::File(_) => None,
    }
  }

  /// The mention as a draft holds it: `@KON-12`, `@KON-12#9`, ``@`src/lib.rs` ``.
  pub fn written(&self) -> String {
    match self {
      Self::Task(key) => format!("{AT}{key}"),
      Self::Run(key, number) => format!("{AT}{key}{RUN_MARK}{}", number.get()),
      Self::File(path) => format!("{AT}{PATH_QUOTE}{path}{PATH_QUOTE}"),
    }
  }
}

/// Whether a file's path can be written as a mention: one that holds a backtick or a line break cannot.
pub fn writable_path(path: &str) -> bool {
  !path.contains([PATH_QUOTE, '\n', '\r'])
}

/// A mention a text holds, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
  pub range: Range<usize>,
  pub mention: Mention,
}

/// Every mention `text` holds, in order: an `@` that starts a word, then a task key and, for a run, `#` and its number,
/// or a file's path between backticks; with no letter, digit, `_` or `-` right after. An `@` inside a word, as in an
/// email address, starts none, and neither does one inside a mention.
pub fn find(text: &str) -> Vec<Found> {
  let mut found: Vec<Found> = Vec::new();
  for (at, _) in text.match_indices(AT) {
    if found.last().is_some_and(|last| at < last.range.end) || !starts_word(text, at) {
      continue;
    }
    if let Some((end, mention)) = mention_at(text, at + AT.len_utf8()) {
      found.push(Found {
        range: at..end,
        mention,
      });
    }
  }
  found
}

/// The mention that ends at `caret` in `text`, which Backspace there removes whole.
pub fn ending_at(text: &str, caret: usize) -> Option<Found> {
  find(text).into_iter().find(|found| found.range.end == caret)
}

/// What the person types after an `@` for the menu: where the `@` is and the words after it up to the caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Typed<'a> {
  pub at: usize,
  pub query: &'a str,
}

impl Typed<'_> {
  /// What a choice of the menu replaces: the `@` and what was typed after it.
  pub const fn range(&self) -> Range<usize> {
    self.at..self.at + AT.len_utf8() + self.query.len()
  }
}

/// What the caret at `caret` of `text` types for the menu: an `@` that starts a word, with no space between it and
/// the caret. A whole mention ending at the caret is a chip already, and types nothing.
pub fn typed_at(text: &str, caret: usize) -> Option<Typed<'_>> {
  let before = text.get(..caret)?;
  let at = before.rfind(AT)?;
  let query = &before[at + AT.len_utf8()..];
  if query.chars().any(char::is_whitespace) || !starts_word(text, at) {
    return None;
  }
  if mention_at(text, at + AT.len_utf8()).is_some_and(|(end, _)| end == caret) {
    return None;
  }
  Some(Typed { at, query })
}

/// Whether a mention may start at `at`: at the start of the text, or after a space or an opening bracket.
fn starts_word(text: &str, at: usize) -> bool {
  text[..at]
    .chars()
    .next_back()
    .is_none_or(|before| before.is_whitespace() || "([{".contains(before))
}

/// The mention that starts at `start` of `text`, right after its `@`, and where it ends.
fn mention_at(text: &str, start: usize) -> Option<(usize, Mention)> {
  let (end, mention) = if text[start..].starts_with(PATH_QUOTE) {
    file_at(text, start)?
  } else {
    task_or_run_at(text, start)?
  };
  let ends_word = text[end..]
    .chars()
    .next()
    .is_none_or(|after| !(after.is_alphanumeric() || after == '_' || after == '-' || after == RUN_MARK));
  ends_word.then_some((end, mention))
}

/// The task or run whose task key starts at `start` of `text`, and where it ends.
fn task_or_run_at(text: &str, start: usize) -> Option<(usize, Mention)> {
  let rest = &text[start..];
  let prefix = rest.find(|ch: char| !(ch.is_ascii_uppercase() || ch.is_ascii_digit()))?;
  let number_len = digits(rest[prefix..].strip_prefix('-')?);
  if prefix == 0 || number_len == 0 {
    return None;
  }
  let key_end = start + prefix + 1 + number_len;
  let written = &text[start..key_end];
  let key: TaskKey = written
    .parse()
    .ok()
    .filter(|key: &TaskKey| key.to_string() == written)?;
  Some(match text[key_end..].strip_prefix(RUN_MARK) {
    Some(number) if digits(number) > 0 => {
      let end = key_end + RUN_MARK.len_utf8() + digits(number);
      let number: NonZeroU32 = text[key_end + RUN_MARK.len_utf8()..end].parse().ok()?;
      (end, Mention::Run(key, RunNumber::new(number)))
    }
    _ => (key_end, Mention::Task(key)),
  })
}

/// The file whose path, between backticks, starts with the backtick at `start` of `text`, and where it ends: a path
/// as the Files screen takes it, written as it reads ([`FilePath`]), on one line.
fn file_at(text: &str, start: usize) -> Option<(usize, Mention)> {
  let inner = start + PATH_QUOTE.len_utf8();
  let length = text[inner..].find([PATH_QUOTE, '\n', '\r'])?;
  let written = &text[inner..inner + length];
  if !text[inner + length..].starts_with(PATH_QUOTE) {
    return None;
  }
  let path = FilePath::new(written).ok().filter(|path| path.as_str() == written)?;
  Some((inner + length + PATH_QUOTE.len_utf8(), Mention::File(path)))
}

/// How many ASCII digits `text` starts with.
fn digits(text: &str) -> usize {
  text.find(|ch: char| !ch.is_ascii_digit()).unwrap_or(text.len())
}
