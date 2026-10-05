//! Globs for the file tools without ripgrep, read as ripgrep reads `--glob`: `*` and `?` within one name,
//! `**` across folders, `{a,b}` alternatives; a glob without `/` matches a file's name at any depth.

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
  pub(super) fn new(pattern: &str) -> Self {
    let pattern = pattern.trim().replace('\\', "/");
    let whole_path = pattern.contains('/');
    let pattern = pattern.trim_start_matches('/');
    Self {
      alternatives: expand(pattern).iter().map(|one| tokens(one)).collect(),
      whole_path,
    }
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

/// `pattern` with its first `{a,b}` group expanded, recursively.
fn expand(pattern: &str) -> Vec<String> {
  let Some(open) = pattern.find('{') else {
    return vec![pattern.to_owned()];
  };
  let Some(close) = pattern[open..].find('}').map(|at| open + at) else {
    return vec![pattern.to_owned()];
  };
  let (head, tail) = (&pattern[..open], &pattern[close + 1..]);
  pattern[open + 1..close]
    .split(',')
    .flat_map(|choice| expand(&format!("{head}{choice}{tail}")))
    .collect()
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

fn matches(tokens: &[Token], subject: &[char]) -> bool {
  let Some((first, rest)) = tokens.split_first() else {
    return subject.is_empty();
  };
  match first {
    Token::Literal(expected) => subject.first() == Some(expected) && matches(rest, &subject[1..]),
    Token::One => subject.first().is_some_and(|c| *c != '/') && matches(rest, &subject[1..]),
    Token::Any => (0..=subject.len())
      .take_while(|end| *end == 0 || subject[end - 1] != '/')
      .any(|end| matches(rest, &subject[end..])),
    Token::AnyDeep => (0..=subject.len()).any(|end| matches(rest, &subject[end..])),
    Token::Folders => {
      matches(rest, subject)
        || subject
          .iter()
          .enumerate()
          .filter(|(_, c)| **c == '/')
          .any(|(slash, _)| matches(rest, &subject[slash + 1..]))
    }
  }
}
