//! Usage while the period is read (`11 Usage · loading` `P74Xv`): the controls, the figures' names, the cards' titles
//! and switches stay; the figures' values and changes, the legend, the y axis, the bars and the plan limits' rows are
//! bones in their own geometry. The bars sit low and even, so they promise no shape. Tokens and plan limits are read
//! separately, so each card loses its bones on its own. *Reading Kontur's token usage…* (`usage.loading.tokens`) and
//! `usage.loading.limits` name them.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::ExtraTypography,
  ui::skeleton::{self, Skeleton, bone, fading, round_bone},
};

/// The chart's bars: how many, each one's width and height, low and even (`P74Xv`).
const BARS: usize = 7;
const BAR: (f32, f32) = (36.0, 44.0);
/// The y axis' ticks and their bones, and a day's bone under its bar.
const TICKS: usize = 5;
const TICK: (f32, f32) = (30.0, 8.0);
const DAY: f32 = 36.0;
/// A plan limit's rows: its name's bone.
const LIMITS: [f32; 4] = [152.0, 120.0, 174.0, 100.0];

/// The tokens card's body while its figures are read, named `label`.
pub(super) fn tokens(ctx: &mut Ctx, label: Arc<str>) -> Element {
  let legend = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(round_bone(10.0))
    .child(skeleton::line(ctx, TypographyStyle::Caption, 120.0));
  let axis = Column::new()
    .height(Dimension::Pct(100.0))
    .justify(Justify::SpaceBetween)
    .with_children((0..TICKS).map(|_| bone(TICK.0, TICK.1)));
  let bars = Row::new()
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .justify(Justify::SpaceAround)
    .align_items(Alignment::End)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children((0..BARS).map(|_| bone(BAR.0, BAR.1)));
  let days = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(Dimension::Px(TICK.0)))
    .justify(Justify::SpaceAround)
    .with_children((0..BARS).map(|_| skeleton::line(ctx, TypographyStyle::Caption, DAY)));
  let chart = Row::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .spacing(SpacingSize::Md)
    .child(axis)
    .child(bars);
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(legend)
    .child(chart)
    .child(days);
  Skeleton::new("usage-tokens-reading", label).fills().show(ctx, bones)
}

/// The plan limits card's body while they are read, its rows fading.
pub(super) fn limits(ctx: &mut Ctx) -> Element {
  let rows = LIMITS
    .iter()
    .map(|name| {
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::vertical(SpacingSize::Md))
        .spacing(SpacingSize::Sm)
        .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(bone(14.0, 14.0))
            .child(skeleton::line(ctx, TypographyStyle::Description, *name))
            .child(Spacer::new().flex(1.0))
            .child(skeleton::line(ctx, TypographyStyle::Caption, 36.0)),
        )
        .child(bone(1.0, 6.0).width(Dimension::Pct(100.0)))
        .child(skeleton::line(ctx, TypographyStyle::Caption, 162.0))
    })
    .collect();
  let bones = Column::new().width(Dimension::Pct(100.0)).with_children(fading(rows));
  Skeleton::new("usage-limits-reading", ctx.t("usage.loading.limits"))
    .fills()
    .show(ctx, bones)
}

/// A figure's value, change and what it counts while the period is read, named `label`, under its real name.
pub(super) fn figure(ctx: &mut Ctx, id: &str, label: Arc<str>) -> Element {
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(skeleton::line(ctx, ExtraTypography::Display.role(), 70.0))
    .child(skeleton::line(ctx, TypographyStyle::Caption, 140.0))
    .child(skeleton::line(ctx, TypographyStyle::Caption, 116.0));
  Skeleton::new(format!("{id}-reading"), label).show(ctx, bones)
}
