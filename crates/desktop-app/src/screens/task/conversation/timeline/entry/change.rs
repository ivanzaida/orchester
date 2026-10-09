//! The change a CLI's or a native agent's call of Orchester's `edit_file` or `write_file` made, under its open
//! row: on `code_bg`, each edit's lines on the diff's backgrounds with their change mark, a `border` rule
//! between edits, a note on an edit that replaces every occurrence or changes line endings or the final line
//! break, and above them how many lines are left out. Lines wrap. They are the call's own text, not the
//! file's: they carry no line numbers.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::stream::{CallChange, ChangeLine, FinalBreak},
  diff::LineKind,
};

use crate::theme::{ExtraColor, ExtraSpacing};

/// Width of the change mark.
const MARK_WIDTH: f32 = 10.0;

/// The change's block, named after `id`.
pub(super) fn block(ctx: &Ctx, change: &CallChange, id: &str) -> Element {
  let omitted = (change.omitted() > 0).then(|| {
    let words = if change.omitted() == 1 {
      ctx.t("conversation.change.trimmed.one")
    } else {
      ctx.t_args(
        "conversation.change.trimmed.other",
        [("count", change.omitted().to_string().as_str())],
      )
    };
    Text::new(&words).variant(TypographyStyle::Caption)
  });
  let hunks = change.hunks().iter().enumerate().map(|(index, hunk)| {
    let notes = [
      hunk.every().then_some("conversation.change.every"),
      hunk.breaks().line_endings.then_some("conversation.change.lineEndings"),
      hunk.breaks().final_break.map(|change| match change {
        FinalBreak::Added => "conversation.change.finalBreakAdded",
        FinalBreak::Removed => "conversation.change.finalBreakRemoved",
      }),
    ];
    let lines = Column::new()
      .width(Dimension::Pct(100.0))
      .with_children(notes.into_iter().flatten().map(|key| note(ctx, key)))
      .with_children(hunk.lines().iter().map(|line| change_line(ctx, line)));
    if index == 0 {
      lines
    } else {
      lines.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    }
  });
  Column::new()
    .id(format!("{id}-change"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(omitted)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .background(ExtraColor::CodeBackground.role())
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Sm)
        .clip()
        .with_children(hunks),
    )
    .into()
}

fn change_line(ctx: &Ctx, line: &ChangeLine) -> Element {
  let (background, mark, mark_color, kind) = match line.kind() {
    LineKind::Context => (
      ExtraColor::CodeBackground.role(),
      None,
      PaletteColor::TextMuted,
      "context",
    ),
    LineKind::Added => (
      ExtraColor::DiffAdded.role(),
      Some("conversation.change.markAdded"),
      PaletteColor::Success,
      "added",
    ),
    LineKind::Removed => (
      ExtraColor::DiffRemoved.role(),
      Some("conversation.change.markRemoved"),
      PaletteColor::Danger,
      "removed",
    ),
  };
  let mark = Text::new(&mark.map(|key| ctx.t(key)).unwrap_or_default())
    .variant(TypographyStyle::Mono)
    .color(mark_color)
    .nowrap()
    .width(Dimension::Px(MARK_WIDTH))
    .flex_shrink(0.0);
  let code = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .child(
      Text::new(line.text())
        .width(Dimension::Pct(100.0))
        .variant(TypographyStyle::Mono)
        .selectable(true),
    )
    .with_children(
      line
        .cut()
        .then(|| Text::new(&ctx.t("conversation.change.lineCut")).variant(TypographyStyle::Caption)),
    );
  Row::new()
    .describe("kind", kind)
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Start)
    .padding_horizontal(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .background(background)
    .child(mark)
    .child(code)
    .into()
}

/// What an edit's lines do not show: that it replaces every occurrence, or changes line endings.
fn note(ctx: &Ctx, key: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Sm)
    .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption))
    .into()
}
