//! @-mentions of the project's tasks and runs in the Orchestrator's and a task Conversation's composer (ORC-312;
//! `docs/design/mentions.md`, `Mentions · redesign` `z8cUk`). A mention is written into the draft as plain text,
//! `@KON-12` for a task and `@KON-12#9` for its run #9, so a draft stays the plain string both composers keep and
//! a composer draws each mention as a chip. The @ menu offers the project's tasks and runs ([`catalog`]); a message
//! sent with mentions carries a marked block of what Orchester knows of each after the person's words ([`facts`]),
//! which the conversations leave out of what they show.

pub mod catalog;
pub mod facts;

use std::{num::NonZeroU32, ops::Range};

use orchester_core::{project::TaskKey, run::RunNumber};

/// What starts a mention.
const AT: char = '@';
/// What puts a run's number after its task's key.
const RUN_MARK: char = '#';

/// What a mention points at: a task by its key, or a run by its task's key and its number.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mention {
  Task(TaskKey),
  Run(TaskKey, RunNumber),
}

impl Mention {
  /// The task the mention names, or whose run it names.
  pub const fn task(&self) -> &TaskKey {
    match self {
      Self::Task(key) | Self::Run(key, _) => key,
    }
  }

  /// The mention as a draft holds it: `@KON-12`, `@KON-12#9`.
  pub fn written(&self) -> String {
    match self {
      Self::Task(key) => format!("{AT}{key}"),
      Self::Run(key, number) => format!("{AT}{key}{RUN_MARK}{}", number.get()),
    }
  }
}

/// A mention a text holds, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
  pub range: Range<usize>,
  pub mention: Mention,
}

/// Every mention `text` holds, in order: an `@` that starts a word, a task key and, for a run, `#` and its number,
/// with no letter, digit, `_` or `-` right after. An `@` inside a word, as in an email address, starts none.
pub fn find(text: &str) -> Vec<Found> {
  text
    .match_indices(AT)
    .filter(|(at, _)| starts_word(text, *at))
    .filter_map(|(at, _)| {
      mention_at(text, at + AT.len_utf8()).map(|(end, mention)| Found {
        range: at..end,
        mention,
      })
    })
    .collect()
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

/// The mention whose task key starts at `start` of `text`, and where it ends.
fn mention_at(text: &str, start: usize) -> Option<(usize, Mention)> {
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
  let (end, mention) = match text[key_end..].strip_prefix(RUN_MARK) {
    Some(number) if digits(number) > 0 => {
      let end = key_end + RUN_MARK.len_utf8() + digits(number);
      let number: NonZeroU32 = text[key_end + RUN_MARK.len_utf8()..end].parse().ok()?;
      (end, Mention::Run(key, RunNumber::new(number)))
    }
    _ => (key_end, Mention::Task(key)),
  };
  let ends_word = text[end..]
    .chars()
    .next()
    .is_none_or(|after| !(after.is_alphanumeric() || after == '_' || after == '-' || after == RUN_MARK));
  ends_word.then_some((end, mention))
}

/// How many ASCII digits `text` starts with.
fn digits(text: &str) -> usize {
  text.find(|ch: char| !ch.is_ascii_digit()).unwrap_or(text.len())
}
