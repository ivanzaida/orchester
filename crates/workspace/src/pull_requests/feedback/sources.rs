//! The places in the code an agent's answer points to, as its thread lists them (`Sources` in `qJKSC`): a file
//! path with a line, written `path:12`, `path, line 12` or `path line 12`. Only paths with a file extension count,
//! so ordinary words and numbers are never taken for code.

/// A file and line an answer names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRef {
  pub path: String,
  pub line: u32,
}

/// The most places a thread lists.
const MAX_SOURCES: usize = 4;

/// The places `text` names, in the order it names them, each once.
pub fn sources(text: &str) -> Vec<SourceRef> {
  let mut found: Vec<SourceRef> = Vec::new();
  let words: Vec<&str> = text.split_whitespace().collect();
  for (at, word) in words.iter().enumerate() {
    let word = word.trim_matches(|c: char| matches!(c, '`' | '(' | ')' | '"' | '\'' | '.' | ';'));
    let (path, line) = match word.split_once(':') {
      Some((path, line)) => (path, number(line)),
      None => (word.trim_end_matches(','), following_line(&words[at + 1..])),
    };
    let Some(line) = line.filter(|_| is_path(path)) else {
      continue;
    };
    let source = SourceRef {
      path: path.to_owned(),
      line,
    };
    if !found.contains(&source) {
      found.push(source);
    }
    if found.len() == MAX_SOURCES {
      break;
    }
  }
  found
}

/// The line `line 12` right after a path names.
fn following_line(rest: &[&str]) -> Option<u32> {
  match rest {
    [word, line, ..] if word.eq_ignore_ascii_case("line") => number(line),
    _ => None,
  }
}

fn number(text: &str) -> Option<u32> {
  let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
  digits.parse().ok().filter(|line| *line > 0)
}

/// A relative file path with an extension: `src/settings.rs`, `settings.rs`.
fn is_path(text: &str) -> bool {
  let Some((stem, extension)) = text.rsplit_once('.') else {
    return false;
  };
  !stem.is_empty()
    && !extension.is_empty()
    && extension.chars().all(|c| c.is_ascii_alphanumeric())
    && extension.chars().any(|c| c.is_ascii_alphabetic())
    && text
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))
}
