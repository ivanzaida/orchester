//! The tokens the file's lines are drawn with: plain text at once, the highlighting once a background thread
//! made it, and for a file read again before that, its last highlighting on the lines whose text did not change.
//! A line is laid out to [`MAX_SHOWN_CHARS`] shown characters; a longer one is cut there and says so, so a long
//! minified line or a one-line file of megabytes does not shape on the UI thread. Each is worked out once per
//! text and kept while its `Arc` is the one shown.

use std::sync::Arc;

use orchester_workspace::{
  files::{FilePath, FileText},
  highlight::{LineTokens, plain, shorten},
};
use parking_lot::Mutex;

use super::line::{CODE_CHAR_WIDTH, GAP_WIDTH, NUMBER_WIDTH};

/// The most characters of a line that are laid out.
pub const MAX_SHOWN_CHARS: usize = 4_000;

/// A text's lines as plain tokens, which of them were cut, and the width its longest shown line needs.
#[derive(Clone)]
pub struct Plain {
  pub tokens: Arc<Vec<LineTokens>>,
  pub cut: Arc<[bool]>,
  pub width: f32,
}

/// The highlighting a file had, and the colors made of it for a text read again.
struct Carried {
  plain: Arc<Vec<LineTokens>>,
  highlighted: Arc<Vec<LineTokens>>,
  carried: Arc<Vec<LineTokens>>,
}

/// The pane's line tokens, kept per text.
#[derive(Default)]
pub struct ShownLines {
  plain: Mutex<Option<(Arc<FileText>, Plain)>>,
  highlighted: Mutex<Option<(FilePath, Arc<Vec<LineTokens>>)>>,
  carried: Mutex<Option<Carried>>,
}

impl ShownLines {
  /// The lines of `text` as plain text, cut to [`MAX_SHOWN_CHARS`].
  pub fn plain(&self, text: &Arc<FileText>) -> Plain {
    let mut kept = self.plain.lock();
    if let Some((made_for, made)) = kept.as_ref()
      && Arc::ptr_eq(made_for, text)
    {
      return made.clone();
    }
    let (tokens, cut): (Vec<LineTokens>, Vec<bool>) = text.lines().iter().map(|line| plain_line(line)).unzip();
    let longest = tokens.iter().map(shown_chars).max().unwrap_or(0);
    let made = Plain {
      tokens: Arc::new(tokens),
      cut: cut.into(),
      width: content_width(longest),
    };
    *kept = Some((text.clone(), made.clone()));
    made
  }

  /// The tokens to draw `file`'s `text` with: `highlighted` once it is made; before, while the file was
  /// highlighted when it was read last, those colors on the lines whose text did not change and plain text on
  /// the rest; else plain text.
  pub fn colors(
    &self,
    (file, text): (&FilePath, &Arc<FileText>),
    highlighted: Option<&Arc<Vec<LineTokens>>>,
  ) -> Arc<Vec<LineTokens>> {
    if let Some(tokens) = highlighted {
      *self.highlighted.lock() = Some((file.clone(), tokens.clone()));
      return tokens.clone();
    }
    let plain_lines = self.plain(text).tokens;
    let before = self.highlighted.lock();
    let Some((_, before)) = before.as_ref().filter(|(shown, _)| shown == file) else {
      return plain_lines;
    };
    let mut carried = self.carried.lock();
    if let Some(made) = carried.as_ref()
      && Arc::ptr_eq(&made.plain, &plain_lines)
      && Arc::ptr_eq(&made.highlighted, before)
    {
      return made.carried.clone();
    }
    let lines: Arc<Vec<LineTokens>> = Arc::new(
      plain_lines
        .iter()
        .enumerate()
        .map(|(index, line)| match before.get(index) {
          Some(colored) if same_text(colored, line) => colored.clone(),
          _ => line.clone(),
        })
        .collect(),
    );
    *carried = Some(Carried {
      plain: plain_lines,
      highlighted: before.clone(),
      carried: lines.clone(),
    });
    lines
  }
}

/// `line` as plain tokens cut to [`MAX_SHOWN_CHARS`] shown characters, and whether it was cut. Only its first
/// characters are made into tokens, however long it is.
fn plain_line(line: &str) -> (LineTokens, bool) {
  let (head, longer) = match line.char_indices().nth(MAX_SHOWN_CHARS) {
    Some((at, _)) => (&line[..at], true),
    None => (line, false),
  };
  let (tokens, shortened) = shorten(&plain(head), MAX_SHOWN_CHARS);
  (tokens, longer || shortened)
}

/// Highlighted lines cut as plain ones are ([`MAX_SHOWN_CHARS`]).
pub fn cut_lines(lines: Vec<LineTokens>) -> Vec<LineTokens> {
  lines
    .into_iter()
    .map(|line| shorten(&line, MAX_SHOWN_CHARS).0)
    .collect()
}

fn shown_chars(line: &LineTokens) -> usize {
  line.iter().map(|token| token.text.chars().count()).sum()
}

/// Whether two lines' tokens show the same text.
fn same_text(a: &LineTokens, b: &LineTokens) -> bool {
  a.iter()
    .flat_map(|token| token.text.chars())
    .eq(b.iter().flat_map(|token| token.text.chars()))
}

/// The width a line of `chars` characters needs.
#[allow(
  clippy::cast_precision_loss,
  reason = "a line holds far fewer characters than f32 counts exactly"
)]
fn content_width(chars: usize) -> f32 {
  NUMBER_WIDTH + GAP_WIDTH + chars as f32 * CODE_CHAR_WIDTH
}
