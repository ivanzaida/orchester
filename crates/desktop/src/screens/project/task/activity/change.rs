//! The change of a CLI's call of Orchester's `edit_file` or `write_file`, under its open row: in the output block's
//! frame (`bjcu3`), the lines of each edit on the diff's backgrounds with their change mark (`vEZ42`: `code-bg`,
//! `diff-add-*`, `diff-remove-*`), a hairline between edits, and above them how many lines are left out. Lines wrap:
//! a row has no room to scroll sideways.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, RadiusSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::stream::{CallChange, ChangeLine},
  diff::LineKind,
};

use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography};

/// Width of the change mark (`K3NKFG`).
const MARK_WIDTH: f32 = 10.0;

/// The change's block, named after the entry `sequence`.
pub(super) fn change_block(ctx: &Ctx, change: &CallChange, sequence: u64) -> Element {
  let omitted = (change.omitted > 0).then(|| {
    Text::new(&ctx.t_args(
      "runs.activity.changeTrimmed",
      [("count", change.omitted.to_string().as_str())],
    ))
    .variant(TypographyStyle::Caption)
  });
  let hunks = change.hunks.iter().enumerate().map(|(index, lines)| {
    let hunk = Column::new()
      .width(Dimension::Pct(100.0))
      .with_children(lines.iter().map(|line| change_line(ctx, line)));
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

fn change_line(ctx: &Ctx, line: &ChangeLine) -> Element {
  let (background, mark, mark_color, kind) = match line.kind {
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
      Text::new(&line.text)
        .variant(ExtraTypography::Code.role())
        .selectable(true),
    )
    .with_children(line.cut.then(|| {
      Text::new(&ctx.t("diff.lineCut"))
        .variant(TypographyStyle::Caption)
        .color(ExtraColor::CodeComment.role())
    }));
  Row::new()
    .id("activity-change-line")
    .describe("kind", kind)
    .width(Dimension::Pct(100.0))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Inline.role())
    .background(background.role())
    .child(mark)
    .child(code)
    .into()
}
