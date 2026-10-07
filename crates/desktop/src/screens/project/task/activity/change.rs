//! The change of a CLI's call of Orchester's `edit_file` or `write_file`, under its open row: in the output block's
//! frame (`bjcu3`), the lines of each edit on the diff's backgrounds with their change mark (`vEZ42`: `code-bg`,
//! `diff-add-*`, `diff-remove-*`), a hairline between edits, a note on an edit that replaces every occurrence, or
//! changes line endings or the final line break, and above them how many lines are left out. Lines wrap: a row has
//! no room to scroll sideways. The lines are the call's own text, not the file's: they carry no line numbers.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, RadiusSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::stream::{CallChange, ChangeLine, FinalBreak},
  diff::LineKind,
};

use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography};

/// Width of the change mark (`K3NKFG`).
const MARK_WIDTH: f32 = 10.0;
/// Height of a line on one line (`vEZ42`); a longer line wraps and grows.
const LINE_HEIGHT: f32 = 24.0;

/// The change's block, named after the entry `sequence`.
pub(super) fn change_block(ctx: &Ctx, change: &CallChange, sequence: u64) -> Element {
  let omitted = (change.omitted() > 0).then(|| {
    let key = if change.omitted() == 1 {
      "runs.activity.changeTrimmed.one"
    } else {
      "runs.activity.changeTrimmed.other"
    };
    Text::new(&ctx.t_args(key, [("count", change.omitted().to_string().as_str())])).variant(TypographyStyle::Caption)
  });
  let hunks = change.hunks().iter().enumerate().map(|(index, hunk)| {
    let notes = [
      hunk.every().then_some("runs.activity.change.every"),
      hunk.breaks().line_endings.then_some("runs.activity.change.lineEndings"),
      hunk.breaks().final_break.map(|change| match change {
        FinalBreak::Added => "runs.activity.change.finalBreakAdded",
        FinalBreak::Removed => "runs.activity.change.finalBreakRemoved",
      }),
    ];
    let hunk = Column::new()
      .width(Dimension::Pct(100.0))
      .with_children(notes.into_iter().flatten().map(|key| note(ctx, key)))
      .with_children(hunk.lines().iter().enumerate().map(|(at, line)| {
        let first: usize = change.hunks()[..index].iter().map(|hunk| hunk.lines().len()).sum();
        change_line(ctx, line, format!("activity-change-text-{sequence}-{}", first + at))
      }));
    if index == 0 {
      hunk
    } else {
      hunk.border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    }
  });
  Column::new()
    .id(format!("activity-entry-change-{sequence}"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(omitted)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .background(ExtraColor::CodeBackground.role())
        .border_inside(BorderSize::Sm, ExtraColor::Hairline.role())
        .rounded(RadiusSize::Sm)
        .clip()
        .with_children(hunks),
    )
    .into()
}

/// Line `line`, its code named `id`.
fn change_line(ctx: &Ctx, line: &ChangeLine, id: String) -> Element {
  let (background, mark, mark_color, kind) = match line.kind() {
    LineKind::Context => (ExtraColor::CodeBackground, None, ExtraColor::CodeComment, "context"),
    LineKind::Added => (
      ExtraColor::DiffAddedBackground,
      Some("diff.markAdded"),
      ExtraColor::DiffAddedMark,
      "added",
    ),
    LineKind::Removed => (
      ExtraColor::DiffRemovedBackground,
      Some("diff.markRemoved"),
      ExtraColor::DiffRemovedMark,
      "removed",
    ),
  };
  let mark = Text::new(&mark.map(|key| ctx.t(key)).unwrap_or_default())
    .variant(ExtraTypography::Code.role())
    .color(mark_color.role())
    .nowrap()
    .width(Dimension::Px(MARK_WIDTH))
    .flex_shrink(0.0);
  let code = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .child(
      // The whole row's width, not the text's own: a box exactly as wide as its text can wrap it when painted at a
      // fractional scale (`.tmp/lurq-wrap-report.md`).
      Text::new(line.text())
        .id(id)
        .width(Dimension::Pct(100.0))
        .variant(ExtraTypography::Code.role())
        .selectable(true),
    )
    .with_children(line.cut().then(|| {
      Text::new(&ctx.t("diff.lineCut"))
        .variant(TypographyStyle::Caption)
        .color(ExtraColor::CodeComment.role())
    }));
  Row::new()
    .id("activity-change-line")
    .describe("kind", kind)
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(LINE_HEIGHT))
    .align_items(Alignment::Center)
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Inline.role())
    .background(background.role())
    .child(mark)
    .child(code)
    .into()
}

/// What a hunk's lines do not show: that the edit replaces every occurrence, or changes line endings.
fn note(ctx: &Ctx, key: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .child(
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::Caption)
        .color(ExtraColor::CodeComment.role()),
    )
    .into()
}
