//! Searching the words of History's conversations (ORC-307, `docs/design/history-search.md`): where a plain text is
//! said in a conversation's messages, the person's, the Orchestrator's and what a daily report's turn added, case and
//! accents ignored as the transcript search ignores them ([`Needle`]). For each conversation it says how many places
//! and messages hold it, and the line History shows under the title: who said it and the words around the first
//! place, on one line, with the entry the conversation opens at. Titles are not searched here: History filters them
//! at once.

use std::{ops::Range, path::Path};

use orchester_core::{
  id::ChatId,
  orchestrator::{EntryKind, TurnMode},
};
use orchester_store::ProjectDb;

use crate::{
  mentions::facts,
  messages::{self, Message},
  tasks::timeline::{find::Needle, view::Rendered},
};

/// How many characters of what comes before the first place a matching line keeps, at most.
const BEFORE: usize = 24;
/// How many characters a matching line keeps from where it starts; History cuts it shorter to its width.
const LINE: usize = 160;
/// What a matching line shows where it cuts the words off.
const CUT: char = '…';

/// Who said a matching line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speaker {
  /// The person.
  Person,
  /// The Orchestrator, answering the person or an agent's wait.
  Orchestrator,
  /// The Orchestrator, in the turn a daily report started by itself.
  Report,
}

/// The line History shows under a conversation's title: the words around the first place, on one line, cut with
/// [`CUT`] where they go on, the places it holds (`marks`, its bytes), and the entry it is in (`seq`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchingLine {
  pub who: Speaker,
  pub text: String,
  pub marks: Vec<Range<usize>>,
  pub seq: i64,
}

/// Where a conversation's messages hold the text: how many places, in how many messages, and the first one's line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InMessages {
  pub chat: ChatId,
  pub places: usize,
  pub messages: usize,
  pub line: MatchingLine,
}

/// The conversations of `chats`, of the project at `root`, whose messages hold `needle`, in the order of `chats`;
/// Markdown is searched as `rendered` shows it. Blocks on SQLite.
pub fn search_in(
  root: &Path,
  chats: &[ChatId],
  needle: &Needle,
  rendered: Rendered<'_>,
) -> Result<Vec<InMessages>, Message> {
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let mut found = Vec::new();
  for &chat in chats {
    let entries = db.chat_entries(chat).map_err(|error| messages::store_error(&error))?;
    found.extend(in_messages(
      chat,
      entries.iter().map(|entry| (entry.seq, &entry.kind)),
      needle,
      rendered,
    ));
  }
  Ok(found)
}

/// Where the entries of `chat`, by their `seq`, hold `needle`; `None` when none does.
fn in_messages<'a>(
  chat: ChatId,
  entries: impl Iterator<Item = (i64, &'a EntryKind)>,
  needle: &Needle,
  rendered: Rendered<'_>,
) -> Option<InMessages> {
  let mut answering = Speaker::Orchestrator;
  let (mut places, mut messages, mut line) = (0, 0, None);
  for (seq, kind) in entries {
    let (who, text) = match kind {
      // The message that carries out an approved plan holds the plan as the agent is told it, which the chat does not
      // show: it is no words of the person's (ORC-646).
      EntryKind::Person {
        mode: TurnMode::CarriesOut,
        ..
      } => {
        answering = Speaker::Orchestrator;
        continue;
      }
      EntryKind::Person { text, .. } => {
        answering = Speaker::Orchestrator;
        (Speaker::Person, facts::said(text.as_str()))
      }
      EntryKind::Reply { text, .. } => (answering, text.as_str()),
      EntryKind::Report { .. } => {
        answering = Speaker::Report;
        continue;
      }
      EntryKind::Wait { .. } | EntryKind::WatchEnded { .. } => {
        answering = Speaker::Orchestrator;
        continue;
      }
      EntryKind::Action(_) | EntryKind::Event(_) | EntryKind::Plan(_) => continue,
    };
    let shown = rendered(text).unwrap_or_else(|| text.to_owned());
    let flat = one_line(&shown);
    let found = needle.places(&flat);
    let Some(first) = found.first() else {
      continue;
    };
    places += found.len();
    messages += 1;
    if line.is_none() {
      let text = around(&flat, first.start);
      line = Some(MatchingLine {
        who,
        marks: needle.places(&text),
        text,
        seq,
      });
    }
  }
  line.map(|line| InMessages {
    chat,
    places,
    messages,
    line,
  })
}

/// `text` on one line: every run of whitespace one space.
fn one_line(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The words of `line` around the place at byte `at`: from the start of a word at most [`BEFORE`] characters before
/// it, [`LINE`] characters at most, cut with [`CUT`] at either end where the words go on.
fn around(line: &str, at: usize) -> String {
  let before: Vec<(usize, char)> = line[..at].char_indices().collect();
  let start = if before.len() <= BEFORE {
    0
  } else {
    let from = before[before.len() - BEFORE].0;
    // The first whole word after where it may start; the place itself when no space comes before it.
    line[from..at].find(' ').map_or(at, |space| from + space + 1)
  };
  let kept = &line[start..];
  let end = kept.char_indices().nth(LINE).map_or(kept.len(), |(byte, _)| byte);
  let mut text = String::new();
  if start > 0 {
    text.push(CUT);
  }
  text.push_str(&kept[..end]);
  if end < kept.len() {
    text.push(CUT);
  }
  text
}
