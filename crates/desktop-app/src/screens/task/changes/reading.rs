//! The Changes tab while the run's changes are read (`04 Task · Changes · loading` `J9qwsb`): the toolbar keeps its
//! frame and its marks while the base, the checkpoint, the totals and *Try locally* are bones; the file list and the
//! diff keep their panes (the diff on `code_bg`, its file head) and show bones in the rows' own heights, 30 for a file
//! and 22 for a line, the lower ones fading, with no added or removed tint until a line is known. *Reading the run's
//! changes* (`changes.loading`) names it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::files::FILE_LIST_WIDTH;
use crate::{
  theme::{ExtraColor, IconSize},
  ui::{
    icon::{Icon, icon},
    skeleton::{self, Skeleton, bone, fading, round_bone},
  },
};

/// The toolbar's height (`lX6yt`), a file row's and a diff line's (`J9qwsb`), and a line number's column.
const TOOLBAR_HEIGHT: f32 = 44.0;
const FILE_ROW: f32 = 30.0;
const DIFF_LINE: f32 = 22.0;
const NUMBER: f32 = 14.0;
/// The files' path bones, and the diff's line bones (0 for a blank line).
const FILES: [f32; 6] = [162.0, 184.0, 152.0, 206.0, 120.0, 172.0];
const LINES: [f32; 18] = [
  252.0, 196.0, 326.0, 284.0, 0.0, 152.0, 348.0, 304.0, 228.0, 0.0, 206.0, 382.0, 260.0, 186.0, 326.0, 240.0, 0.0, 174.0,
];

/// The tab's toolbar and panes in bones.
pub fn bones(ctx: &mut Ctx) -> Element {
  let panes = Row::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .align_items(Alignment::Start)
    .child(files(ctx))
    .child(diff(ctx));
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .child(toolbar(ctx))
    .child(panes);
  Skeleton::new("changes-loading", ctx.t("changes.loading"))
    .fills()
    .show(ctx, bones)
}

/// The toolbar: its marks real, the base, the checkpoint, the totals and *Try locally* bones.
fn toolbar(ctx: &Ctx) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Section))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::GitCompare, IconSize::Base, PaletteColor::TextMuted))
    .child(skeleton::line(ctx, TypographyStyle::Mono, 98.0))
    .child(icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::TextMuted))
    .child(skeleton::line(ctx, TypographyStyle::Mono, 52.0))
    .child(skeleton::line(ctx, TypographyStyle::Description, 220.0))
    .child(Spacer::new().flex(1.0))
    .child(skeleton::line(ctx, TypographyStyle::Mono, 120.0))
    .child(skeleton::button(96.0))
}

/// The file list's pane with its rows' bones: a status mark, the path and the counts.
fn files(ctx: &Ctx) -> Column {
  let rows = FILES
    .iter()
    .map(|path| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(FILE_ROW))
        .padding(Padding::horizontal(SpacingSize::Md))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(round_bone(8.0))
        .child(skeleton::line(ctx, TypographyStyle::Description, *path))
        .child(Spacer::new().flex(1.0))
        .child(skeleton::count())
    })
    .collect();
  Column::new()
    .width(Dimension::Px(FILE_LIST_WIDTH))
    .padding(Padding::vertical(SpacingSize::Sm))
    .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(fading(rows))
}

/// The diff's pane on `code_bg`: its file head, then its lines' numbers and words as bones.
fn diff(ctx: &Ctx) -> Column {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(skeleton::line(ctx, TypographyStyle::Mono, 290.0))
    .child(skeleton::count())
    .child(Spacer::new().flex(1.0))
    .child(bone(NUMBER, NUMBER));
  let lines = LINES
    .iter()
    .map(|words| {
      let line = Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(DIFF_LINE))
        .padding(Padding::horizontal(SpacingSize::Md))
        .spacing(SpacingSize::Xl)
        .align_items(Alignment::Center)
        .child(bone(NUMBER, 8.0));
      if *words > 0.0 {
        line.child(skeleton::line(ctx, TypographyStyle::Mono, *words))
      } else {
        line
      }
    })
    .collect();
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .background(ExtraColor::CodeBackground.role())
    .child(head)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::vertical(SpacingSize::Sm))
        .with_children(fading(lines)),
    )
}
