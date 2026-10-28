//! A block's inline content: its text and the marks over it, the shape lurq's styled `text_input` reports (spans over
//! text), so the editor maps one to the other directly.

use std::cmp::Reverse;

use super::BlocksError;

/// A style over part of a block's text.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MarkKind {
  Bold,
  Italic,
  /// Inline code. No other mark starts or ends inside it: Markdown has no marks in code.
  Code,
  Strikethrough,
  /// A link to its target as the Markdown writes it: an address, a path, an Orchester link.
  Link(String),
}

/// `kind` over the bytes `start..end` of a block's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
  pub start: usize,
  pub end: usize,
  pub kind: MarkKind,
}

/// A block's text with its marks. A line break in the text is `\n` (a hard break in Markdown); soft breaks are spaces.
///
/// The marks are kept canonical, so two texts that look the same compare equal: no empty mark, marks of one kind (and
/// links of one target) that touch or overlap are one mark, and they are ordered by start, the longer first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineText {
  text: String,
  marks: Vec<Mark>,
}

impl InlineText {
  /// Text without marks.
  pub fn plain(text: impl Into<String>) -> Self {
    Self {
      text: text.into(),
      marks: Vec::new(),
    }
  }

  /// `text` with `marks`, made canonical. Refused when a mark lies outside the text or off a character boundary.
  pub fn new(text: impl Into<String>, marks: Vec<Mark>) -> Result<Self, BlocksError> {
    let text = text.into();
    if let Some(mark) = marks
      .iter()
      .find(|mark| mark.start > mark.end || !text.is_char_boundary(mark.start) || !text.is_char_boundary(mark.end))
    {
      return Err(BlocksError::MarkOutsideText {
        start: mark.start,
        end: mark.end,
        len: text.len(),
      });
    }
    Ok(Self::canonical(text, marks))
  }

  /// `text` with `marks` the parser made, which lie inside it by construction.
  pub(super) fn canonical(text: String, mut marks: Vec<Mark>) -> Self {
    marks.retain(|mark| mark.start < mark.end);
    marks.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.start.cmp(&b.start)));
    let mut merged: Vec<Mark> = Vec::with_capacity(marks.len());
    for mark in marks {
      match merged.last_mut() {
        Some(last) if last.kind == mark.kind && mark.start <= last.end => last.end = last.end.max(mark.end),
        _ => merged.push(mark),
      }
    }
    merged.sort_by(|a, b| (a.start, Reverse(a.end), &a.kind).cmp(&(b.start, Reverse(b.end), &b.kind)));
    Self { text, marks: merged }
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  pub fn marks(&self) -> &[Mark] {
    &self.marks
  }

  pub fn is_empty(&self) -> bool {
    self.text.is_empty()
  }
}
