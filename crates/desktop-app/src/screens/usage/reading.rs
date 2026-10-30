//! Usage while the period is read (`11 Usage · loading` `P74Xv`): the controls, the figures' names, the cards' titles
//! and switches stay; the figures' values and changes, the legend, the y axis, the bars and the plan limits' rows are
//! bones in their own geometry. The bars sit low and even, so they promise no shape. What the bars count, the days
//! under them and the chart's footnote stay real: the period says them. Tokens and plan limits are read separately,
//! so each card loses its bones on its own. *Reading Kontur's token usage…* (`usage.loading.tokens`) and
//! `usage.loading.limits` name them.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  time::Timestamp,
  usage::{Grouping, Measure, local_date},
};
use orchester_workspace::usage::UsageQuery;

use super::{
  card::{SWATCH, footnote},
  plot::{AXIS_GUTTER, AxisName, CHART_HEIGHT, LABEL_GAP, LABEL_LINE, PLOT_HEIGHT, TOP_ROOM},
  tokens_legend::{ITEM_HEIGHT, LEGEND_HEIGHT, caption_key},
  words,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, UsageSpacing},
  ui::skeleton::{self, Skeleton, bone, fading, round_bone},
};

/// A bar's widest and its height, low and even (`P74Xv`).
const BAR: (f32, f32) = (36.0, 44.0);
/// The y axis' ticks and their bones, right-aligned 8 before the plot.
const TICKS: usize = 5;
const TICK: (f32, f32) = (30.0, 8.0);
const TICK_GAP: f32 = 8.0;
/// The most day names under the bars; more days name every few, counted back from the last.
const DAY_NAMES: usize = 10;
/// A plan limit's rows: its name's bone.
const LIMITS: [f32; 4] = [152.0, 120.0, 174.0, 100.0];

/// The tokens card's body while its figures are read, named `label`: the legend's and the chart's bones in their
/// loaded room, what the bars count for `measure` and the days of `query`'s period real, then the footnote.
pub(super) fn tokens(ctx: &mut Ctx, label: Arc<str>, (query, measure): (&UsageQuery, Measure)) -> Vec<Element> {
  let today = local_date(Timestamp::now());
  let range = query.range(today);
  let grouping = query.grouping(range);
  let legend = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LEGEND_HEIGHT))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Row::new()
        .height(Dimension::Px(ITEM_HEIGHT))
        .padding(Padding::new().bottom(ExtraSpacing::Snug.role()))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(round_bone(SWATCH))
        .child(skeleton::line(ctx, TypographyStyle::Description, 120.0)),
    )
    .child(
      Text::new(&ctx.t(caption_key(measure, grouping)))
        .id("usage-tokens-caption")
        .variant(TypographyStyle::Caption)
        .nowrap(),
    );
  let names: Vec<Option<AxisName>> = match grouping {
    Grouping::Hour if range.days() == 1 => (0..24)
      .map(|hour| (hour % 3 == 0).then(|| AxisName::plain(words::hour(hour).into())))
      .collect(),
    Grouping::Hour | Grouping::Day => {
      let dates: Vec<_> = range.dates().collect();
      let every = dates.len().div_ceil(DAY_NAMES).max(1);
      let last = dates.len().saturating_sub(1);
      dates
        .iter()
        .enumerate()
        .map(|(index, date)| {
          (last - index)
            .is_multiple_of(every)
            .then(|| AxisName::day(ctx, *date, today))
        })
        .collect()
    }
  };
  let chart = chart(&names);
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(UsageSpacing::AboveChart.role())
    .child(legend)
    .child(chart);
  let room = LEGEND_HEIGHT + UsageSpacing::AboveChart.value() + CHART_HEIGHT;
  let skeleton = Skeleton::new("usage-tokens-reading", label).show(ctx, bones);
  vec![
    Column::new()
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(room))
      .child(skeleton)
      .into(),
    footnote("usage-tokens-footnote", &ctx.t("usage.tokens.footnote")),
  ]
}

/// The chart in the canvas' room: the y axis' bones, a low even bar in each slot over the baseline, and the slots'
/// names under them, real.
fn chart(names: &[Option<AxisName>]) -> Column {
  let axis = Column::new()
    .width(Dimension::Px(AXIS_GUTTER))
    .height(Dimension::Pct(100.0))
    .padding(Padding::new().right(Dimension::Px(TICK_GAP)))
    .justify(Justify::SpaceBetween)
    .align_items(Alignment::End)
    .with_children((0..TICKS).map(|_| bone(TICK.0, TICK.1)));
  let slot = || {
    Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .align_items(Alignment::Center)
  };
  let bars = Row::new()
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::End)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(names.iter().map(|_| {
      slot().child(
        bone(BAR.0, BAR.1)
          .width(Dimension::Pct(60.0))
          .max_width(Dimension::Px(BAR.0)),
      )
    }));
  let days = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LABEL_LINE))
    .padding(Padding::new().left(Dimension::Px(AXIS_GUTTER)))
    .with_children(names.iter().map(|name| {
      slot().with_children(name.as_ref().map(|name| {
        let color = if name.today {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextMuted
        };
        Text::new(&name.text)
          .variant(TypographyStyle::Caption)
          .color(color)
          .nowrap()
      }))
    }));
  Column::new()
    .id("usage-tokens-chart-bones")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(CHART_HEIGHT))
    .padding(Padding::new().top(Dimension::Px(TOP_ROOM - TICK.1 / 2.0)))
    .spacing(Dimension::Px(LABEL_GAP))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(PLOT_HEIGHT + TICK.1 / 2.0))
        .child(axis)
        .child(bars),
    )
    .child(days)
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
