//! One line of the file, mounted by the virtualised list: a diff's context line without its old number and
//! sign (`Diff line/context` `L5WYtA` as `Line 440` uses it): 20 high, the number right-aligned in 44 in
//! Geist Mono 11 `text_muted`, 24 of room, then the code in Geist Mono 12, its tokens in the Changes tab's
//! colors. A selected line is on `accent_muted` with a 2 `accent` line at its start and its number in
//! `text_primary` (`Line 441` `dSRWm`). A place the search found is on `warning_muted`, the current one on
//! `warning` in `text_inverse`, radius 2 (`Match`, `Current match` in `b7RGL`).
//!
//! The code is sensitive text: it is painted, but the MCP and DevTools trees show it redacted and their
//! screenshots cover it. A press on the number selects the line, with Shift the lines from the selection's
//! first to it.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Button, Row, Text},
  layout::{Alignment, text_style::TextAlign},
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_workspace::highlight::{LineTokens, Token};

use crate::theme::{ExtraTypography, FilesRadius, token_color};

/// Heights and widths of a line (`L5WYtA`): its height, its number's column and the room after it.
pub const LINE_HEIGHT: f32 = 20.0;
pub const NUMBER_WIDTH: f32 = 44.0;
pub const GAP_WIDTH: f32 = 24.0;
/// Advance of one Geist Mono character at 12: 0.6 em.
pub const CODE_CHAR_WIDTH: f32 = 7.2;

/// A place found in the line: characters `start..end`, and whether it is the current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
  pub start: usize,
  pub end: usize,
  pub current: bool,
}

/// One line as the list mounts it.
#[derive(Clone, PartialEq)]
pub struct CodeLine {
  /// The line's index, from 0; its key in the list.
  pub index: usize,
  pub tokens: LineTokens,
  pub selected: bool,
  pub found: Arc<[Found]>,
}

/// Selects the line at an index; `true` extends the selection to it.
pub type Select = Arc<dyn Fn(usize, bool) + Send + Sync>;

#[derive(Clone)]
pub struct LineProps {
  pub line: CodeLine,
  /// Every line is this wide: the longest line's code or the pane, so a selection reaches across.
  pub width: f32,
  pub select: Select,
}

impl PartialEq for LineProps {
  fn eq(&self, other: &Self) -> bool {
    self.line == other.line && self.width == other.width
  }
}

impl DevtoolsInspectable for LineProps {}

pub struct LineView;

impl Component for LineView {
  type Props = LineProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let LineProps { line, width, select } = ctx.props::<LineProps>().clone();
    let index = line.index;
    let number_color = if line.selected {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextMuted
    };
    let number = Button::empty()
      .id(format!("files-line-number-{}", index + 1))
      .focusable(false)
      .cursor(CursorIcon::Pointer)
      .width(Dimension::Px(NUMBER_WIDTH))
      .height(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .on_click(move |event: MouseEvent| select(index, event.shift))
      .child(
        Text::new(&(index + 1).to_string())
          .variant(ExtraTypography::MonoNote.role())
          .color(number_color)
          .text_align(TextAlign::Right)
          .width(Dimension::Pct(100.0))
          .nowrap(),
      );
    let row = Row::new()
      .id(format!("files-line-{}", index + 1))
      .describe("selected", line.selected.to_string())
      .describe("found", line.found.len().to_string())
      .width(Dimension::Px(width))
      .height(Dimension::Px(LINE_HEIGHT))
      .align_items(Alignment::Center)
      .child(number)
      .child(Row::new().width(Dimension::Px(GAP_WIDTH)))
      .with_children(code(&line.tokens, &line.found));
    if line.selected {
      row
        .background(PaletteColor::AccentMuted)
        .border_left(Border::inside(BorderSize::Md, PaletteColor::Accent))
    } else {
      row
    }
  }
}

/// The line's code as sensitive texts, split where a found place starts and ends.
fn code(tokens: &[Token], found: &[Found]) -> Vec<Element> {
  let mut parts = Vec::new();
  let mut column = 0;
  for token in tokens {
    let chars: Vec<char> = token.text.chars().collect();
    let mut at = 0;
    while at < chars.len() {
      let here = column + at;
      let place = found.iter().find(|place| place.start <= here && here < place.end);
      let boundary = match place {
        Some(place) => place.end,
        None => found
          .iter()
          .map(|place| place.start)
          .filter(|start| *start > here)
          .min()
          .unwrap_or(usize::MAX),
      };
      let end = (boundary - column).min(chars.len());
      let text: String = chars[at..end].iter().collect();
      parts.push(piece(&text, token, place));
      at = end;
    }
    column += chars.len();
  }
  parts
}

fn piece(text: &str, token: &Token, place: Option<&Found>) -> Element {
  let shown = Text::new(text).sensitive().variant(TypographyStyle::Mono).nowrap();
  match place {
    None => shown.color(token_color(token.kind)).into(),
    Some(place) => {
      let (background, color) = if place.current {
        (PaletteColor::Warning, PaletteColor::TextInverse)
      } else {
        (PaletteColor::WarningMuted, token_color(token.kind))
      };
      Row::new()
        .background(background)
        .rounded(FilesRadius::Found.role())
        .child(shown.color(color))
        .into()
    }
  }
}
