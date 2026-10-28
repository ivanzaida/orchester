//! The paths in a report's texts. Folders are matched as either platform writes them: `\` and `/` alike, and
//! ASCII letters in any case, which on macOS hides more than it has to but never less.

use std::path::Path;

const OUTSIDE: &str = "<path>";

pub(crate) struct PathRules {
  source_root: Option<Folder>,
  home: Option<Folder>,
}

impl PathRules {
  pub(crate) fn new(source_root: &Path, home: Option<&Path>) -> Self {
    Self {
      source_root: Folder::new(source_root, Shortening::Relative),
      home: home.and_then(|home| Folder::new(home, Shortening::Home)),
    }
  }

  /// `text` with the source root's paths made relative, the home folder's paths shortened to `~` with what
  /// follows it hidden, and every other absolute path hidden. The source root goes first because it may lie in
  /// the home folder.
  pub(crate) fn rewrite(&self, text: &str) -> String {
    let mut text = text.to_owned();
    for folder in [&self.source_root, &self.home].into_iter().flatten() {
      text = folder.replace(&text);
    }
    hide_absolute(&text)
  }
}

enum Shortening {
  /// The folder becomes nothing, with the separator after it, so a source file reads relative to it.
  Relative,
  /// The folder becomes `~`, and a path inside it `~/<path>`, so no folder or file name in it shows.
  Home,
}

struct Folder {
  /// The folder in [`comparable`] form, without a trailing separator.
  key: String,
  shortening: Shortening,
}

impl Folder {
  fn new(path: &Path, shortening: Shortening) -> Option<Self> {
    let key = comparable(&path.to_string_lossy());
    let key = key.trim_end_matches('/');
    (!key.is_empty()).then(|| Self {
      key: key.to_owned(),
      shortening,
    })
  }

  fn replace(&self, text: &str) -> String {
    let comparable = comparable(text);
    let mut replaced = String::with_capacity(text.len());
    let mut copied = 0;
    for (start, _) in comparable.match_indices(&self.key) {
      let end = start + self.key.len();
      if start < copied || continues_name(&text[end..]) {
        continue;
      }
      replaced.push_str(&text[copied..start]);
      copied = self.shorten(text, end, &mut replaced);
    }
    replaced.push_str(&text[copied..]);
    replaced
  }

  /// Pushes the shortened form of the folder that ends at `end` in `text`, with what follows it when that is
  /// hidden too, and returns where the text goes on.
  fn shorten(&self, text: &str, end: usize, replaced: &mut String) -> usize {
    let rest = &text[end..];
    let separator = rest.chars().next().filter(|c| matches!(c, '/' | '\\'));
    match (&self.shortening, separator) {
      (Shortening::Relative, Some(_)) => end + 1,
      (Shortening::Relative, None) => end,
      (Shortening::Home, Some(separator)) if word_len(&rest[1..]) > 0 => {
        replaced.push('~');
        replaced.push(separator);
        replaced.push_str(OUTSIDE);
        end + 1 + path_len(&rest[1..])
      }
      (Shortening::Home, _) => {
        replaced.push('~');
        end
      }
    }
  }
}

/// Whether `rest`, the text after a folder, goes on with a longer folder name (`C:\Users\bob` in
/// `C:\Users\bobby` or `C:\Users\bob.smith`).
fn continues_name(rest: &str) -> bool {
  rest
    .chars()
    .next()
    .is_some_and(|next| next.is_alphanumeric() || matches!(next, '_' | '-' | '.'))
}

/// `text` with `\` as `/` and ASCII letters in lower case. Every character keeps its length in bytes, so
/// offsets in it are offsets in `text`.
fn comparable(text: &str) -> String {
  text
    .chars()
    .map(|c| if c == '\\' { '/' } else { c.to_ascii_lowercase() })
    .collect()
}

/// `text` with every absolute path that starts after a space, a quote, a bracket, `=`, `:` or a URL's `://`
/// replaced by [`OUTSIDE`].
fn hide_absolute(text: &str) -> String {
  let mut hidden = String::with_capacity(text.len());
  let mut copied = 0;
  let mut index = 0;
  let mut after_opening = true;
  while let Some(c) = text[index..].chars().next() {
    if after_opening && starts_path(&text[index..]) {
      let end = index + path_len(&text[index..]);
      hidden.push_str(&text[copied..index]);
      hidden.push_str(OUTSIDE);
      copied = end;
      index = end;
      after_opening = false;
      continue;
    }
    if text[index..].starts_with("://") {
      // `file:///D:/work` and `file:///Users/x`: the path starts after the URL's scheme.
      index += "://".len();
      after_opening = true;
      continue;
    }
    after_opening = c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '(' | '[' | '<' | '{' | '=' | ',' | ';' | ':');
    index += c.len_utf8();
  }
  hidden.push_str(&text[copied..]);
  hidden
}

/// Whether `text` starts with a Windows drive path (`C:\`, `C:/`), a UNC path (`\\server`) or a Unix one
/// (`/Users`).
fn starts_path(text: &str) -> bool {
  let is_separator = |b: u8| b == b'\\' || b == b'/';
  match text.as_bytes() {
    [drive, b':', separator, ..] => drive.is_ascii_alphabetic() && is_separator(*separator),
    [b'\\', b'\\', next, ..] => !is_separator(*next) && !next.is_ascii_whitespace(),
    [b'/', next, ..] => next.is_ascii_alphanumeric() || matches!(next, b'.' | b'_' | b'~' | b'-'),
    _ => false,
  }
}

/// The length of the path `text` starts with. A Windows path goes on past a space while the next word holds a
/// `\` (`C:\Program Files\App`).
fn path_len(text: &str) -> usize {
  let mut end = word_len(text);
  if text.starts_with('/') {
    return end;
  }
  while text[end..].starts_with(' ') {
    let next = end + 1;
    let word = word_len(&text[next..]);
    if word == 0 || !text[next..next + word].contains('\\') {
      break;
    }
    end = next + word;
  }
  end
}

fn word_len(text: &str) -> usize {
  text
    .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | ')' | ']' | '>' | '}' | ',' | ';'))
    .unwrap_or(text.len())
}
