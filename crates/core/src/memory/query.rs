//! What an agent's step is about, as terms to search memory with. No embedding model: records are
//! found by their words (ranked by BM25) and by fragments of text (trigram substrings), which also
//! find other forms of a word, file paths and identifiers inside longer ones.

use std::collections::BTreeSet;

/// Search terms made from a step's and its task's text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryQuery {
  words: Vec<String>,
  fragments: Vec<String>,
}

impl MemoryQuery {
  /// At most this many words and as many fragments, the first ones of the text.
  pub const MAX_TERMS: usize = 64;
  /// Terms shorter than this match too much; a trigram index cannot look them up at all.
  const MIN_CHARS: usize = 3;
  /// Words at least this long lose their last two letters, which is where inflection happens
  /// (Ukrainian `міграція`, `міграції`, `міграцію` share `міграц`; English `decision`, `decisions`).
  const STEM_FROM: usize = 5;

  pub fn from_text(text: &str) -> Self {
    let words = distinct(
      text
        .split(|character: char| !character.is_alphanumeric())
        .map(|word| stem(&word.to_lowercase())),
    );
    let fragments = distinct(
      text
        .split_whitespace()
        .map(|fragment| fragment.trim_matches(|character: char| !is_fragment_char(character)))
        .map(|fragment| stem(&fragment.to_lowercase())),
    );
    Self { words, fragments }
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

fn distinct(terms: impl Iterator<Item = String>) -> Vec<String> {
  let mut seen = BTreeSet::new();
  terms
    .filter(|term| term.chars().count() >= MemoryQuery::MIN_CHARS)
    .filter(|term| seen.insert(term.clone()))
    .take(MemoryQuery::MAX_TERMS)
    .collect()
}
