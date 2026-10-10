//! One row of a file's diff, mounted by the virtualised list. A hunk heading (`Diff hunk` `sphVU`): 26 high
//! on `hover`, its range in Geist Mono 11.5 `text_muted`, 12 from the side. A line (`Diff line/context`
//! `L5WYtA`, `/remove` `tWgTT`, `/add` `C8FPa`): 20 high, its old and new numbers right-aligned in 44 wide
//! columns in Geist Mono 11 `text_muted`, its sign centered in 24 (`+` in `success`, `−` in `danger`), then
//! its code in Geist Mono 12, a removed line on `diff_remove` and an added one on `diff_add`; tokens take
//! their colors from `theme::token_color`. A line cut short says so after its code; a note
//! about what the diff does not show reads like the cut row of a pull request's diff (`Diff cut row`
//! `S2U6g`). Rows are read-only.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::diff::LineKind;
use orchester_workspace::highlight::TokenKind;

use super::rows::{DiffRow, FileNote, LineRow};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, token_color},
  ui::icon::{Icon, icon},
};

/// Heights of a diff line and a hunk heading (`L5WYtA`, `sphVU`).
const LINE_HEIGHT: f32 = 20.0;
const HUNK_HEIGHT: f32 = 26.0;
/// Widths of a line number column and of the sign (`ey7iM`, `OtSVu`).
const NUMBER_WIDTH: f32 = 44.0;
const SIGN_WIDTH: f32 = 24.0;
/// Advance of one Geist Mono character at 12 (code) and 11.5 (hunk headings): 0.6 em.
const CODE_CHAR_WIDTH: f32 = 7.2;
const HUNK_CHAR_WIDTH: f32 = 6.9;
/// A hunk heading's padding at its sides (`sphVU`).
const HUNK_PADDING: f32 = 12.0;
/// Room for what follows a line's code ("Line shortened", "No newline at end of file").
const MARKER_WIDTH: f32 = 200.0;

/// A row, drawn `width` wide: the widest row of the file or the pane, whichever is wider, so every row's
/// background reaches across while long lines scroll sideways.
#[derive(Clone, PartialEq)]
pub struct RowProps {
  pub row: DiffRow,
  pub width: f32,
}

impl lurq::app::component::DevtoolsInspectable for RowProps {}

pub struct DiffRowView;

impl Component for DiffRowView {
  type Props = RowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RowProps { row, width } = ctx.props::<RowProps>().clone();
    let width = Dimension::Px(width);
    match row {
      DiffRow::Hunk { old, new, section, .. } => hunk(ctx, (&old, &new, &section), width),
      DiffRow::Line(line) => line_row(ctx, &line, width),
      DiffRow::Note { note, .. } => note_row(ctx, &note, width),
      DiffRow::ScrollbarGap { .. } => {
        let scrollbar = ctx.theme().scrollbar().clone();
        let height = scrollbar.width + 2.0 * scrollbar.resolved_edge_inset();
        Row::new()
          .id("changes-diff-scrollbar-gap")
          .width(width)
          .height(Dimension::Px(height))
          .into()
      }
    }
  }
}

/// The width the widest of `rows` needs.
pub fn content_width(rows: &[DiffRow]) -> f32 {
  rows.iter().map(row_width).fold(0.0, f32::max)
}

#[allow(
  clippy::cast_precision_loss,
  reason = "a line holds at most `MAX_LINE_CHARS` characters, far inside f32's exact range"
)]
fn row_width(row: &DiffRow) -> f32 {
  match row {
    DiffRow::Hunk { old, new, section, .. } => {
      // "@@ -old +new @@ section".
      let chars = old.chars().count() + new.chars().count() + section.chars().count() + 10;
      2.0 * HUNK_PADDING + chars as f32 * HUNK_CHAR_WIDTH
    }
    DiffRow::Line(line) => {
      let chars: usize = line.tokens.iter().map(|token| token.text.chars().count()).sum();
      let marker = if line.cut || line.no_newline { MARKER_WIDTH } else { 0.0 };
      2.0 * NUMBER_WIDTH + SIGN_WIDTH + chars as f32 * CODE_CHAR_WIDTH + marker
    }
    DiffRow::Note { .. } | DiffRow::ScrollbarGap { .. } => 0.0,
  }
}

fn hunk(ctx: &Ctx, (old, new, section): (&str, &str, &str), width: Dimension) -> Element {
  let text = if section.is_empty() {
    ctx.t_args("changes.diff.hunk", [("old", old), ("new", new)])
  } else {
    ctx.t_args(
      "changes.diff.hunkSection",
      [("old", old), ("new", new), ("section", section)],
    )
  };
  Row::new()
    .id("changes-diff-hunk")
    .width(width)
    .height(Dimension::Px(HUNK_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .align_items(Alignment::Center)
    .background(ExtraColor::Hover.role())
    .child(Text::new(&text).variant(ExtraTypography::MonoKey.role()).nowrap())
    .into()
}

fn line_row(ctx: &Ctx, line: &LineRow, row_width: Dimension) -> Element {
  let (background, sign, sign_color, kind) = match line.kind {
    LineKind::Context => (None, None, PaletteColor::TextMuted, "context"),
    LineKind::Added => (
      Some(ExtraColor::DiffAdded),
      Some("changes.diff.added"),
      PaletteColor::Success,
      "added",
    ),
    LineKind::Removed => (
      Some(ExtraColor::DiffRemoved),
      Some("changes.diff.removed"),
      PaletteColor::Danger,
      "removed",
    ),
  };
  let number = |value: Option<u32>| {
    Text::new(&value.map(|value| value.to_string()).unwrap_or_default())
      .variant(ExtraTypography::MonoNote.role())
      .text_align(TextAlign::Right)
      .nowrap()
      .width(Dimension::Px(NUMBER_WIDTH))
  };
  let sign = Text::new(&sign.map(|key| ctx.t(key)).unwrap_or_default())
    .variant(TypographyStyle::Mono)
    .color(sign_color)
    .text_align(TextAlign::Center)
    .nowrap()
    .width(Dimension::Px(SIGN_WIDTH));
  let code = line.tokens.iter().map(|token| {
    Text::new(&token.text)
      .class(token_class(token.kind))
      .variant(TypographyStyle::Mono)
      .color(token_color(token.kind))
      .nowrap()
  });
  let mut row = Row::new()
    .id("changes-diff-line")
    .describe("kind", kind)
    .describe("cut", line.cut.to_string())
    .width(row_width)
    .height(Dimension::Px(LINE_HEIGHT))
    .align_items(Alignment::Center)
    .child(number(line.old))
    .child(number(line.new))
    .child(sign)
    .with_children(code);
  if let Some(background) = background {
    row = row.background(background.role());
  }
  if line.cut {
    row = row.child(marker(ctx, "changes-line-cut", Icon::Scissors, "changes.diff.lineCut"));
  }
  if line.no_newline {
    row = row.child(marker(ctx, "changes-no-newline", Icon::Info, "changes.diff.noNewline"));
  }
  row.into()
}

/// What follows a line's code, e.g. that it was cut short: set apart on `surface_raised`.
fn marker(ctx: &Ctx, id: &str, glyph: Icon, key: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .height(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .child(icon(glyph, IconSize::Xs, PaletteColor::TextMuted))
    .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption).nowrap())
    .into()
}

fn note_row(ctx: &Ctx, note: &FileNote, width: Dimension) -> Element {
  let (glyph, text) = match note {
    FileNote::Binary => (Icon::Info, ctx.t("changes.diff.binary")),
    FileNote::Mode { old, new } => (
      Icon::Info,
      ctx.t_args("changes.diff.mode", [("old", &**old), ("new", &**new)]),
    ),
    FileNote::Unchanged { from } => (Icon::Info, ctx.t_args("changes.diff.unchanged", [("path", &**from)])),
    FileNote::Empty => (Icon::Info, ctx.t("changes.diff.empty")),
    FileNote::Omitted(lines) => (
      Icon::Scissors,
      ctx.t_args("changes.diff.omitted", [("count", super::super::grouped(ctx, *lines))]),
    ),
  };
  Row::new()
    .id("changes-diff-note")
    .width(width)
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(Text::new(&text).variant(ExtraTypography::Prose.role()).nowrap())
    .into()
}

/// Names the token's kind for tools (`lurq_read_tree`) and tests.
const fn token_class(kind: TokenKind) -> &'static str {
  match kind {
    TokenKind::Plain => "code-plain",
    TokenKind::Keyword => "code-keyword",
    TokenKind::String => "code-string",
    TokenKind::Comment => "code-comment",
    TokenKind::Constant => "code-constant",
    TokenKind::Function => "code-function",
    TokenKind::Variable => "code-variable",
  }
}
