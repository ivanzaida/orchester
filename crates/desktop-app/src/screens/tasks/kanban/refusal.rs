//! A column's refusal of a card, drawn over its cards and never among them (`Drag overlay` `wjtLk` in `03 Drop
//! refused` `bR7zd`, the refusals of `12 Moves and confirmations` `aUNO8`): while a card is lifted, every column that
//! would refuse it but its own is dimmed by the `scrim` at 72 % with one compact chip near its top, *Run not ended:
//! stop it from the task* (11 `text_primary` after an 11 `ban`, `danger` on the column under the pointer, `text_muted`
//! on the others), on `surface_raised` in a `border_strong` stroke, radius 6, lifted by the menu's shadow; its tooltip
//! says the whole sentence. After a refused drop, or a refused move by the keys or the menu, the column keeps the chip
//! alone until the next move.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Stack, Text},
  layout::{Alignment, StackAlignment},
  node::{Element, HitTestBehavior, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::moves;

use super::actions::Refusal;
use crate::{
  theme::{DROP_SCRIM_OPACITY, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    tooltip::Tooltip,
  },
};

/// The i18n key of why a column refuses a task, for a menu row that cannot be chosen.
pub const fn reason_key(refusal: moves::Refusal) -> &'static str {
  match refusal {
    moves::Refusal::RunNotEnded => "tasks.board.refused.runNotEndedMenu",
    moves::Refusal::Incomplete => "tasks.board.refused.incomplete",
    moves::Refusal::HasRun => "tasks.board.refused.hasRun",
  }
}

/// The whole sentence of a refusal, in the chip's tooltip.
const fn words_key(refusal: &Refusal) -> &'static str {
  match refusal {
    Refusal::RunNotEnded => "tasks.board.refused.runNotEnded",
    Refusal::Incomplete(_) => "tasks.board.refused.incomplete",
    Refusal::HasRun => "tasks.board.refused.hasRun",
    Refusal::Pipeline => "tasks.board.refused.pipeline",
  }
}

/// The chip's few words.
const fn chip_key(refusal: &Refusal) -> &'static str {
  match refusal {
    Refusal::RunNotEnded => "tasks.board.refused.chip.runNotEnded",
    Refusal::Incomplete(_) => "tasks.board.refused.chip.incomplete",
    Refusal::HasRun => "tasks.board.refused.chip.hasRun",
    Refusal::Pipeline => "tasks.board.refused.chip.pipeline",
  }
}

/// How a column says it refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
  /// The pointer is over this column, or the refused drop or move was to it: the chip's mark is `danger`.
  pub hovered: bool,
  /// A card is lifted: the column's cards are dimmed under the chip.
  pub scrim: bool,
}

/// The layer over the cards of the column `id` that refuses for `refusal`: the scrim while a card is lifted, and the
/// chip near its top. It lets the pointer through but for the chip, whose tooltip says why in full. It is a stack,
/// since only a stack places a child by `absolute`: in a column the scrim would have no size.
pub fn layer(ctx: &mut Ctx, id: &str, refusal: &Refusal, shown: Shown) -> Element {
  let chip = chip(ctx, id, refusal, shown.hovered);
  let mut layer = Stack::new()
    .id(format!("{id}-overlay"))
    .stack_align(StackAlignment::TopStart)
    .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
    .hit_test(HitTestBehavior::ContentOnly);
  if shown.scrim {
    layer = layer.child(
      Column::new()
        .id(format!("{id}-scrim"))
        .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
        .rounded(RadiusSize::Md)
        .background(ExtraColor::Scrim.role())
        .opacity(DROP_SCRIM_OPACITY)
        .hit_test(HitTestBehavior::None),
    );
  }
  layer
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().top(SpacingSize::Sm))
        .justify(lurq::layout::layout_kind::Justify::Center)
        .hit_test(HitTestBehavior::ContentOnly)
        .child(chip),
    )
    .into()
}

/// *Run not ended: stop it from the task* (`Refusal chip` `E2djZX`).
fn chip(ctx: &mut Ctx, id: &str, refusal: &Refusal, hovered: bool) -> Element {
  let mark = if hovered {
    PaletteColor::Danger
  } else {
    PaletteColor::TextMuted
  };
  let words: Arc<str> = ctx.t(words_key(refusal));
  let chip = Row::new()
    .id(format!("{id}-refused"))
    .padding(Padding::symmetric(ExtraSpacing::TextArea.role(), SpacingSize::Xs))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .box_shadow(ShadowStyle::Md)
    .describe("a11y_name", words.clone())
    .describe("tone", if hovered { "danger" } else { "muted" })
    .child(icon(Icon::Ban, IconSize::Xs, mark))
    .child(
      Text::new(&ctx.t(chip_key(refusal)))
        .id(format!("{id}-refused-text"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    );
  Tooltip::new(format!("{id}-refused"), words).wrap(ctx, chip)
}
