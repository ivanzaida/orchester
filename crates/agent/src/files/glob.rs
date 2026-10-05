//! Globs for the file tools without ripgrep, read as ripgrep reads `--glob`: `*` and `?` within one name,
//! `**` across folders, `{a,b}` alternatives; a glob without `/` matches a file's name at any depth.
//! A glob is limited in length and in alternatives, and matched without recursion in time proportional
//! to the glob's length times the name's, so that no glob can hang or overflow the stack.

use super::FileError;

/// The longest glob accepted, in characters.
const MAX_CHARS: usize = 256;
/// The most alternatives a glob's `{a,b}` groups may expand to.
const MAX_ALTERNATIVES: usize = 64;

/// A compiled glob.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Glob {
  alternatives: Vec<Vec<Token>>,
  /// The glob names a path (it has a `/`), not a file name.
  whole_path: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
  Literal(char),
  /// `*`: any characters but `/`.
  Any,
  /// `?`: one character but `/`.
  One,
  /// `**` not followed by `/`: any characters.
  AnyDeep,
  /// `**/`: nothing, or any folders.
  Folders,
}

impl Glob {
  pub(super) fn new(pattern: &str) -> Result<Self, FileError> {
    if pattern.chars().count() > MAX_CHARS {
      return Err(FileError::Glob(format!("it is longer than {MAX_CHARS} characters")));
    }
    let pattern = pattern.trim().replace('\\', "/");
    let whole_path = pattern.contains('/');
    let mut alternatives = Vec::new();
    expand(pattern.trim_start_matches('/'), &mut alternatives)?;
    Ok(Self {
      alternatives: alternatives.iter().map(|one| tokens(one)).collect(),
      whole_path,
    })
  }

  /// Whether `relative`, a path with `/` separators, matches.
  pub(super) fn matches(&self, relative: &str) -> bool {
    let subject = if self.whole_path {
      relative
    } else {
      relative.rsplit('/').next().unwrap_or(relative)
    };
    let chars: Vec<char> = subject.chars().collect();
    self.alternatives.iter().any(|tokens| matches(tokens, &chars))
  }
}

/// Adds `pattern` with its `{a,b}` groups expanded to `into`, refusing more than [`MAX_ALTERNATIVES`].
fn expand(pattern: &str, into: &mut Vec<String>) -> Result<(), FileError> {
  let group = pattern
    .find('{')
    .and_then(|open| pattern[open..].find('}').map(|close| (open, open + close)));
  let Some((open, close)) = group else {
    if into.len() >= MAX_ALTERNATIVES {
      return Err(FileError::Glob(format!(
        "its {{a,b}} groups make more than {MAX_ALTERNATIVES} alternatives"
      )));
    }
    into.push(pattern.to_owned());
    return Ok(());
  };
  let (head, tail) = (&pattern[..open], &pattern[close + 1..]);
  for choice in pattern[open + 1..close].split(',') {
    expand(&format!("{head}{choice}{tail}"), into)?;
  }
  Ok(())
}

fn tokens(pattern: &str) -> Vec<Token> {
  let chars: Vec<char> = pattern.chars().collect();
  let mut tokens = Vec::new();
  let mut at = 0;
  while at < chars.len() {
    let token = match (chars[at], chars.get(at + 1), chars.get(at + 2)) {
      ('*', Some('*'), Some('/')) => {
        at += 2;
        Token::Folders
      }
      ('*', Some('*'), _) => {
        at += 1;
        Token::AnyDeep
      }
      ('*', ..) => Token::Any,
      ('?', ..) => Token::One,
      (literal, ..) => Token::Literal(literal),
    };
    tokens.push(token);
    at += 1;
  }
  tokens
}

/// Whether `tokens` match all of `subject`: `next[j]` holds whether the tokens after the current one
/// match `subject[j..]`, filled from the last token to the first.
fn matches(tokens: &[Token], subject: &[char]) -> bool {
  let length = subject.len();
  let mut next: Vec<bool> = (0..=length).map(|at| at == length).collect();
  for token in tokens.iter().rev() {
    let mut row = vec![false; length + 1];
    for at in (0..=length).rev() {
      let here = subject.get(at).copied();
      row[at] = match *token {
        Token::Literal(expected) => here == Some(expected) && next[at + 1],
        Token::One => here.is_some_and(|c| c != '/') && next[at + 1],
        Token::Any => next[at] || (here.is_some_and(|c| c != '/') && row[at + 1]),
        Token::AnyDeep => next[at] || (here.is_some() && row[at + 1]),
        // Nothing, or anything up to and including a `/`: `ends_in_slash` is whether some `/` at or
        // after `at` is followed by a match.
        Token::Folders => next[at] || ends_in_slash(subject, &next, at),
      };
    }
    next = row;
  }
  next[0]
}

fn ends_in_slash(subject: &[char], next: &[bool], from: usize) -> bool {
  subject
    .iter()
    .enumerate()
    .skip(from)
    .any(|(at, c)| *c == '/' && next[at + 1])
}
