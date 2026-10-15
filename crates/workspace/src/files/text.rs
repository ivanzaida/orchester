//! A text file as the code viewer shows it: its lines, a range of them (the `line` of the route), what
//! copying lines gives, and where a text is found in it.

use std::{fmt, str::FromStr, sync::Arc};

use thiserror::Error;

use crate::highlight;

/// At most this many places are found in one file; a search that finds more says so.
pub const MAX_MATCHES: usize = 10_000;

/// The lines of a text file, without their line ends and without a leading byte order mark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileText {
  lines: Arc<[Arc<str>]>,
  /// Bytes in the file.
  size: u64,
}

impl FileText {
  /// The text of `bytes`; bytes that are not UTF-8 show as the replacement character.
  pub fn parse(bytes: &[u8]) -> Self {
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let text = text.strip_suffix('\n').unwrap_or(text);
    let lines = if text.is_empty() {
      Vec::new()
    } else {
      text
        .split('\n')
        .map(|line| Arc::from(line.strip_suffix('\r').unwrap_or(line)))
        .collect()
    };
    Self {
      lines: lines.into(),
      size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    }
  }

  pub fn lines(&self) -> &Arc<[Arc<str>]> {
    &self.lines
  }

  pub const fn size(&self) -> u64 {
    self.size
  }

  /// The text of `range` as copied: its lines joined with `\n`, the part past the file's end left out.
  pub fn copied(&self, range: LineRange) -> String {
    let first = range.first_index();
    let last = range.last_index().min(self.lines.len().saturating_sub(1));
    self
      .lines
      .get(first..=last)
      .map(|lines| lines.join("\n"))
      .unwrap_or_default()
  }

  /// Where `query` is found in the lines as they are shown (tabs as spaces), line by line, at most
  /// [`MAX_MATCHES`]; with `match_case` only in the query's case. Positions count characters of the shown
  /// line.
  pub fn find(&self, query: &str, match_case: bool) -> Vec<TextMatch> {
    let fold = |character: char| if match_case { character } else { folded(character) };
    let query: Vec<char> = query.chars().map(fold).collect();
    if query.is_empty() {
      return Vec::new();
    }
    let mut found = Vec::new();
    for (line, text) in self.lines.iter().enumerate() {
      let shown: Vec<char> = highlight::plain(text)
        .iter()
        .flat_map(|token| token.text.chars())
        .map(fold)
        .collect();
      let mut start = 0;
      while start + query.len() <= shown.len() {
        if shown[start..start + query.len()] == query[..] {
          found.push(TextMatch {
            line,
            start,
            end: start + query.len(),
          });
          if found.len() == MAX_MATCHES {
            return found;
          }
          start += query.len();
        } else {
          start += 1;
        }
      }
    }
    found
  }
}

/// `character` compared without regard to case.
fn folded(character: char) -> char {
  character.to_lowercase().next().unwrap_or(character)
}

/// A place `query` was found: characters `start..end` of the shown line `line` (from 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextMatch {
  pub line: usize,
  pub start: usize,
  pub end: usize,
}

/// Lines `first..=last`, counted from 1, as the route's `line=<a>-<b>` (or `line=<a>`) names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineRange {
  first: u32,
  last: u32,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not a line or a range of lines")]
pub struct LineRangeError(pub String);

impl LineRange {
  /// Lines `a..=b` in either order; `None` when one of them is 0.
  pub fn new(a: u32, b: u32) -> Option<Self> {
    (a > 0 && b > 0).then(|| Self {
      first: a.min(b),
      last: a.max(b),
    })
  }

  pub const fn first(self) -> u32 {
    self.first
  }

  pub const fn last(self) -> u32 {
    self.last
  }

  /// The index of the first line, from 0.
  pub fn first_index(self) -> usize {
    usize::try_from(self.first - 1).unwrap_or(usize::MAX)
  }

  /// The index of the last line, from 0.
  pub fn last_index(self) -> usize {
    usize::try_from(self.last - 1).unwrap_or(usize::MAX)
  }

  /// Whether the line at `index` (from 0) is in the range.
  pub fn contains_index(self, index: usize) -> bool {
    (self.first_index()..=self.last_index()).contains(&index)
  }
}

impl FromStr for LineRange {
  type Err = LineRangeError;

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    let invalid = || LineRangeError(value.to_owned());
    let (a, b) = value.split_once('-').unwrap_or((value, value));
    let a = a.parse().map_err(|_| invalid())?;
    let b = b.parse().map_err(|_| invalid())?;
    Self::new(a, b).ok_or_else(invalid)
  }
}

impl fmt::Display for LineRange {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.first == self.last {
      write!(f, "{}", self.first)
    } else {
      write!(f, "{}-{}", self.first, self.last)
    }
  }
}
