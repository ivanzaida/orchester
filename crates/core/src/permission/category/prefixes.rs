//! Environment variables a command may be prefixed with that change only its encoding, colours or log
//! output (`PYTHONIOENCODING=utf-8 python x.py`, `RUST_LOG=debug cargo test`): an allow-list, each set
//! to a plain literal. Every other variable before a program (`PATH`, `PYTHONPATH`, `NODE_OPTIONS`,
//! `LD_PRELOAD`, `GIT_…`) can change what the program runs or reads, so the part stays Unknown.

use super::variables::is_assignment;

/// The variables a command may be prefixed with.
const BENIGN: [&str; 11] = [
  "PYTHONIOENCODING",
  "PYTHONUTF8",
  "PYTHONUNBUFFERED",
  "NO_COLOR",
  "FORCE_COLOR",
  "TERM",
  "LANG",
  "LC_ALL",
  "RUST_LOG",
  "RUST_BACKTRACE",
  "CARGO_TERM_COLOR",
];

/// `words` without the benign assignments they start with, when a program follows them and the first
/// word's name was written unquoted (`quoted_name`: a quoted `"A=b"` is a program's name to bash).
pub(super) fn without_benign(words: &[String], quoted_name: bool) -> &[String] {
  if quoted_name {
    return words;
  }
  let benign = words.iter().take_while(|word| is_benign(word)).count();
  if benign == words.len() {
    return words;
  }
  &words[benign..]
}

/// `NAME=value` with a listed name and a value of letters, digits and `._,:=+-`.
fn is_benign(word: &str) -> bool {
  is_assignment(word)
    && word.split_once('=').is_some_and(|(name, value)| {
      BENIGN.contains(&name)
        && value
          .chars()
          .all(|c| c.is_ascii_alphanumeric() || "._,:=+-".contains(c))
    })
}
