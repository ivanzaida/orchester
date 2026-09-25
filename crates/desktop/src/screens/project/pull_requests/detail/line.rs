//! One row of a file's diff, mounted by the virtualised list: a hunk heading (`n32O8`), a diff
//! line (`vEZ42`: old and new line numbers, the change mark and the highlighted code on the
//! added, removed or unchanged background) or a note. Rows are read-only and pointer-only; the
//! feedback affordances of `vEZ42` belong to agent feedback, which this screen does not have.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::{Alignment, text_style::TextAlign},
  node::{Element, dimension::Dimension},
};
use orchester_core::pull_request::diff::LineKind;

use super::rows::{DiffRow, FileNote, LineRow};
use crate::{
  pull_requests::highlight::TokenKind,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
};

/// Heights of a diff line and a hunk heading (`vEZ42`, `n32O8`).
pub const LINE_HEIGHT: f32 = 24.0;
const HUNK_HEIGHT: f32 = 32.0;
/// Width of a line number column (`b8bSop`, `U4wNN`: 24 for two digits), and what each further
/// digit of Geist Mono 11 adds.
const NUMBER_WIDTH: f32 = 24.0;
const NUMBER_DIGIT_WIDTH: f32 = 6.6;
/// Width of the change mark (`K3NKFG`).
const MARK_WIDTH: f32 = 10.0;
/// Advance of one Geist Mono character at 12 (code) and 10 (hunk headings).
const CODE_CHAR_WIDTH: f32 = 7.2;
const HUNK_CHAR_WIDTH: f32 = 6.0;
/// Room for a note after a line's code ("Line shortened", "No newline at end of file").
const MARKER_WIDTH: f32 = 180.0;
/// Horizontal padding (`vEZ42` 12, `n32O8` 16) and the gaps between a line's four parts (10).
const LINE_PADDING: f32 = 12.0;
const HUNK_PADDING: f32 = 16.0;
const LINE_GAP: f32 = 10.0;

/// A row, drawn `width` wide: the widest row of the file or the pane, whichever is wider, so
/// every row's background reaches across while long lines scroll sideways.
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
      DiffRow::Hunk { old, new, section, .. } => hunk(ctx, &old, &new, &section, width),
      DiffRow::Line(line) => line_row(ctx, &line, width),
      DiffRow::Note { note, .. } => note_row(ctx, &note, width),
    }
  }
}

/// The width the widest of `rows` needs.
pub fn content_width(rows: &[DiffRow]) -> f32 {
  rows.iter().map(row_width).fold(0.0, f32::max)
}

fn row_width(row: &DiffRow) -> f32 {
  match row {
    DiffRow::Hunk { old, new, section, .. } => {
      // "@@ −old +new @@ section".
      let chars = old.chars().count() + new.chars().count() + section.chars().count() + 10;
      2.0 * HUNK_PADDING + chars as f32 * HUNK_CHAR_WIDTH
    }
    DiffRow::Line(line) => {
      let chars: usize = line.tokens.iter().map(|token| token.text.chars().count()).sum();
      let marker = if line.cut || line.no_newline { MARKER_WIDTH } else { 0.0 };
      2.0 * LINE_PADDING
        + 2.0 * number_width(line.digits)
        + MARK_WIDTH
        + 3.0 * LINE_GAP
        + chars as f32 * CODE_CHAR_WIDTH
        + marker
    }
    DiffRow::Note { .. } => 0.0,
  }
}

fn hunk(ctx: &Ctx, old: &str, new: &str, section: &str, width: Dimension) -> Element {
  let text = if section.is_empty() {
    ctx.t_args("pullRequests.diff.hunk", [("old", old), ("new", new)])
  } else {
    ctx.t_args(
      "pullRequests.diff.hunkSection",
      [("old", old), ("new", new), ("section", section)],
    )
  };
  Row::new()
    .id("pull-request-hunk")
    .width(width)
    .height(Dimension::Px(HUNK_HEIGHT))
    .padding_horizontal(SpacingSize::Md)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .child(Text::new(&text).variant(TypographyStyle::Mono).nowrap())
    .into()
}

fn line_row(ctx: &Ctx, line: &LineRow, row_width: Dimension) -> Element {
  let (background, mark, mark_color, kind) = match line.kind {
    LineKind::Context => (ExtraColor::CodeBackground, None, ExtraColor::CodeComment, "context"),
    LineKind::Added => (
      ExtraColor::DiffAddedBackground,
      Some("pullRequests.diff.markAdded"),
      ExtraColor::DiffAddedMark,
      "added",
    ),
    LineKind::Removed => (
      ExtraColor::DiffRemovedBackground,
      Some("pullRequests.diff.markRemoved"),
      ExtraColor::DiffRemovedMark,
      "removed",
    ),
  };
  let width = number_width(line.digits);
  let number = |value: Option<u32>| {
    Text::new(&value.map(|value| value.to_string()).unwrap_or_default())
      .variant(ExtraTypography::DiffLineNumber.role())
      .text_align(TextAlign::Right)
      .nowrap()
      .width(Dimension::Px(width))
  };
  let mark = Text::new(&mark.map(|key| ctx.t(key)).unwrap_or_default())
    .variant(ExtraTypography::Code.role())
    .color(mark_color.role())
    .nowrap()
    .width(Dimension::Px(MARK_WIDTH));
  let code = line.tokens.iter().map(|token| {
    Text::new(&token.text)
      .class(token_class(token.kind))
      .variant(ExtraTypography::Code.role())
      .color(token_color(token.kind))
      .nowrap()
  });
  let mut code = Row::new().align_items(Alignment::Center).with_children(code);
  if line.cut {
    code = code.child(marker(ctx, "pullRequests.diff.lineCut"));
  }
  if line.no_newline {
    code = code.child(marker(ctx, "pullRequests.diff.noNewline"));
  }
  Row::new()
    .id("pull-request-line")
    .describe("kind", kind)
    .width(row_width)
    .height(Dimension::Px(LINE_HEIGHT))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .background(background.role())
    .child(number(line.old))
    .child(number(line.new))
    .child(mark)
    .child(code)
    .into()
}

/// A note after the code, e.g. that the line was cut; set apart from the code by a gap.
fn marker(ctx: &Ctx, key: &str) -> Element {
  Row::new()
    .padding_left(SpacingSize::Sm)
    .child(
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::Caption)
        .color(ExtraColor::CodeComment.role())
        .nowrap(),
    )
    .into()
}

fn note_row(ctx: &Ctx, note: &FileNote, width: Dimension) -> Element {
  let text = match note {
    FileNote::Binary => ctx.t("pullRequests.diff.binary"),
    FileNote::Mode { old, new } => ctx.t_args("pullRequests.diff.mode", [("old", &**old), ("new", &**new)]),
    FileNote::Unchanged => ctx.t("pullRequests.diff.unchanged"),
    FileNote::Empty => ctx.t("pullRequests.diff.empty"),
    FileNote::Omitted(lines) => ctx.t_args("pullRequests.diff.omitted", [("count", lines.to_string().as_str())]),
  };
  Row::new()
    .id("pull-request-note")
    .width(width)
    .height(Dimension::Px(HUNK_HEIGHT))
    .padding_horizontal(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .child(icon(Icon::Info, IconSize::Small, PaletteColor::TextSecondary))
    .child(Text::new(&text).variant(TypographyStyle::Caption).nowrap())
    .into()
}

fn number_width(digits: usize) -> f32 {
  let extra = digits.saturating_sub(2) as f32;
  NUMBER_WIDTH + extra * NUMBER_DIGIT_WIDTH
}

/// Names the token's kind for tooling (`lurq_read_tree`) and tests.
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

fn token_color(kind: TokenKind) -> PaletteColor {
  let color = match kind {
    TokenKind::Plain => ExtraColor::CodeText,
    TokenKind::Keyword => ExtraColor::CodeKeyword,
    TokenKind::String => ExtraColor::CodeString,
    TokenKind::Comment => ExtraColor::CodeComment,
    TokenKind::Constant => ExtraColor::CodeConstant,
    TokenKind::Function => ExtraColor::CodeFunction,
    TokenKind::Variable => ExtraColor::CodeVariable,
  };
  color.role()
}
