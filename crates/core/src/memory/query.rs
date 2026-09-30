//! What an agent's step is about, as terms to search memory with. No embedding model: records are
//! found by their words (ranked by BM25) and by fragments of text (trigram substrings), which also
//! find other forms of a word, file paths and identifiers inside longer ones. Common English and
//! Ukrainian words are left out: they match everything.

use std::collections::BTreeSet;

/// Search terms made from a step's and its task's text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryQuery {
  words: Vec<String>,
  fragments: Vec<String>,
}

impl MemoryQuery {
  /// At most this many words and as many fragments.
  pub const MAX_TERMS: usize = 64;
  /// At most this many of a step's terms come from its instructions, so long instructions never
  /// crowd out the task's own terms.
  pub const INSTRUCTION_TERMS: usize = 16;
  /// Terms shorter than this match too much; a trigram index cannot look them up at all.
  const MIN_CHARS: usize = 3;
  /// Words at least this long lose their last two letters, which is where inflection happens
  /// (Ukrainian `міграції`, `міграцію` share `міграц`; English `decisions`, `decision` share
  /// `decisi`). Shorter words stay whole: cutting them would match unrelated words.
  const STEM_FROM: usize = 7;

  /// The terms of `text`, the first ones first.
  pub fn from_text(text: &str) -> Self {
    let mut query = Self::default();
    query.add(text, Self::MAX_TERMS);
    query
  }

  /// The terms a step's agent searches with: the task's title and brief first, then the step's
  /// name, then at most [`Self::INSTRUCTION_TERMS`] more from the step's instructions.
  pub fn for_step(title: &str, brief: &str, step: &str, instructions: &str) -> Self {
    let mut query = Self::default();
    for text in [title, brief, step] {
      query.add(text, Self::MAX_TERMS);
    }
    query.add(instructions, Self::INSTRUCTION_TERMS);
    query
  }

  /// Lowercase word stems, each to be matched as a word prefix.
  pub fn words(&self) -> &[String] {
    &self.words
  }

  /// Lowercase fragments (words, paths such as `crates/core/src/run/result.rs`, identifiers such as
  /// `parseStepResult`), each to be matched anywhere in a record's text.
  pub fn fragments(&self) -> &[String] {
    &self.fragments
  }

  pub fn is_empty(&self) -> bool {
    self.words.is_empty() && self.fragments.is_empty()
  }

  /// Adds up to `limit` new words and as many new fragments of `text`, within [`Self::MAX_TERMS`].
  fn add(&mut self, text: &str, limit: usize) {
    let words = text
      .split(|character: char| !character.is_alphanumeric())
      .map(|word| stem(&word.to_lowercase()));
    extend(&mut self.words, words, limit);
    let fragments = text
      .split_whitespace()
      .map(|fragment| fragment.trim_matches(|character: char| !is_fragment_char(character)))
      .map(|fragment| stem(&fragment.to_lowercase()));
    extend(&mut self.fragments, fragments, limit);
  }
}

/// Characters a fragment keeps at its ends: those of words, paths and identifiers.
fn is_fragment_char(character: char) -> bool {
  character.is_alphanumeric() || matches!(character, '_' | '/' | '\\' | '-')
}

fn stem(term: &str) -> String {
  let length = term.chars().count();
  if length >= MemoryQuery::STEM_FROM && term.chars().all(char::is_alphabetic) {
    term.chars().take(length - 2).collect()
  } else {
    term.to_owned()
  }
}

/// Appends to `terms` up to `limit` of `new` that are long enough, not common and not there yet.
fn extend(terms: &mut Vec<String>, new: impl Iterator<Item = String>, limit: usize) {
  let mut seen: BTreeSet<String> = terms.iter().cloned().collect();
  let room = MemoryQuery::MAX_TERMS.saturating_sub(terms.len()).min(limit);
  let added: Vec<String> = new
    .filter(|term| term.chars().count() >= MemoryQuery::MIN_CHARS)
    .filter(|term| !STOP_WORDS.contains(&term.as_str()))
    .filter(|term| seen.insert(term.clone()))
    .take(room)
    .collect();
  terms.extend(added);
}

/// Common English and Ukrainian words of three letters or more.
const STOP_WORDS: &[&str] = &[
  "about",
  "after",
  "again",
  "all",
  "also",
  "and",
  "any",
  "are",
  "before",
  "but",
  "can",
  "does",
  "done",
  "each",
  "every",
  "for",
  "from",
  "has",
  "have",
  "how",
  "into",
  "its",
  "make",
  "more",
  "most",
  "must",
  "not",
  "only",
  "our",
  "out",
  "over",
  "per",
  "should",
  "some",
  "such",
  "sure",
  "than",
  "that",
  "the",
  "their",
  "them",
  "then",
  "there",
  "these",
  "they",
  "this",
  "those",
  "under",
  "use",
  "used",
  "using",
  "via",
  "was",
  "were",
  "what",
  "when",
  "where",
  "which",
  "who",
  "why",
  "will",
  "with",
  "you",
  "your",
  "або",
  "але",
  "буде",
  "було",
  "бути",
  "вже",
  "для",
  "його",
  "коли",
  "має",
  "мають",
  "можна",
  "потрібно",
  "при",
  "про",
  "так",
  "також",
  "треба",
  "цей",
  "щоб",
  "які",
  "який",
  "яка",
  "яке",
  "від",
];
