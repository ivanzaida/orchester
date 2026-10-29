//! A conversation while it is read (`05 Orchestrator · conversation loading` `S65u9m`): the messages load together, so
//! the whole column is bones in the messages' own geometry, your bubbles keeping their `own_message` surface with bone
//! lines, the Orchestrator's answers its avatar, name, lines and action rows, the lower ones fading; no digest and no
//! suggestions while it loads. *Loading conversation…* (`orchestrator.loading.title`) names it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::body::turn_room;
use crate::{
  theme::{ExtraColor, ExtraTypography},
  ui::skeleton::{self, Skeleton, bone, fading, round_bone},
};

/// Your bubble's width (`own_message` in `S65u9m`), the Orchester avatar's side, and an action row's mark.
const BUBBLE: f32 = 566.0;
const AVATAR: f32 = 28.0;
const MARK: f32 = 14.0;

/// One turn of the conversation in bones.
#[derive(Clone, Copy)]
enum Turn {
  /// Your message: its lines.
  Yours(&'static [f32]),
  /// The Orchestrator's answer: its lines and its action rows.
  Answer(&'static [f32], &'static [f32]),
}

/// The turns, oldest first.
const TURNS: [Turn; 5] = [
  Turn::Yours(&[522.0, 326.0]),
  Turn::Answer(&[762.0, 696.0, 304.0], &[240.0, 206.0, 262.0]),
  Turn::Yours(&[392.0]),
  Turn::Answer(&[750.0, 566.0], &[]),
  Turn::Yours(&[478.0, 240.0]),
];

/// The conversation's column while it is read.
pub(super) fn turns(ctx: &mut Ctx) -> Element {
  let turns = TURNS
    .iter()
    .map(|turn| match *turn {
      Turn::Yours(lines) => yours(ctx, lines),
      Turn::Answer(lines, actions) => answer(ctx, lines, actions),
    })
    .collect();
  let column = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Lg))
    .spacing(SpacingSize::Xl)
    .with_children(fading(turns).into_iter().map(|turn| turn_room(ctx, turn)));
  Skeleton::new("orchestrator-loading", ctx.t("orchestrator.loading.title"))
    .fills()
    .show(ctx, column)
}

/// Your message: its bubble at the column's end, on its surface, with bone lines.
fn yours(ctx: &Ctx, lines: &[f32]) -> Column {
  let bubble = Column::new()
    .width(Dimension::Px(BUBBLE))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Lg)
    .background(ExtraColor::OwnMessage.role())
    .with_children(lines.iter().map(|width| skeleton::line(ctx, TypographyStyle::Body, *width)));
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(Row::new().width(Dimension::Pct(100.0)).justify(Justify::End).child(bubble))
}

/// The Orchestrator's answer: its avatar, its name and time, its lines and its action rows.
fn answer(ctx: &Ctx, lines: &[f32], actions: &[f32]) -> Column {
  let rows = actions.iter().map(|width| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(bone(MARK, MARK))
      .child(skeleton::line(ctx, TypographyStyle::Body, *width))
      .child(Spacer::new().flex(1.0))
      .child(skeleton::line(ctx, ExtraTypography::Note.role(), 44.0))
  });
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(skeleton::line(ctx, TypographyStyle::Body, 78.0))
        .child(skeleton::line(ctx, TypographyStyle::Caption, 30.0)),
    )
    .with_children(lines.iter().map(|width| skeleton::line(ctx, TypographyStyle::Body, *width)))
    .with_children(rows);
  Column::new().width(Dimension::Pct(100.0)).child(
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .child(round_bone(AVATAR))
      .child(text),
  )
}
