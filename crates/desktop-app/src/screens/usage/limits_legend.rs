//! What *Plan limits* says of each window (`Limit legend/weekly` `XO1mE`, `Limit legend/5-hour` `p5L7N`):
//! its line (solid for a weekly window, three dashes for a 5-hour one) in the CLI's color, its name in 12
//! `text_secondary`, the share last reported in 12 mono, and under them when it resets, in 11 `text_muted`.
//! Over the chart the 5-hour windows and the weekly ones each take a column (`Legend` `ofKhz`); grouped by
//! day 5-hour windows are not drawn, and their entries are faded to half. Without runs to chart, or when the
//! filters match nothing, the card shrinks to the windows alone and when they were last reported
//! (`Card · Plan limits · compact` `kQuif`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::usage::{
  CliSeries, Grouping,
  limits::{CurrentLimit, WindowKind},
};
use orchester_workspace::usage::{Dashboard, DashboardState};

use super::{
  card::{Card, footnote, series_color},
  limits_chart::line_name,
  words,
};
use crate::theme::{ExtraSpacing, UsageRadius, UsageSpacing};

/// A window's line (`Line` `o7Mlz`): 16 by 8 room, a 2 high stroke, or three 4 wide dashes 2 apart.
const LINE_WIDTH: f32 = 16.0;
const LINE_ROOM: f32 = 8.0;
const LINE_HEIGHT: f32 = 2.0;
const DASHES: usize = 3;
/// What the entries of 5-hour windows keep of their strength while the chart does not draw them.
const UNDRAWN_OPACITY: f32 = 0.5;

/// The windows in legend order: each kind by CLI.
fn windows(dashboard: &Dashboard) -> Vec<&CurrentLimit> {
  let mut windows: Vec<&CurrentLimit> = dashboard.limits.current.iter().collect();
  windows.sort_by_key(|window| (CliSeries::of(Some(window.provider)), window.minutes));
  windows
}

/// The legend over the chart: 5-hour windows in the first column, the others in the second.
pub fn legend(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let (short, long): (Vec<&CurrentLimit>, Vec<&CurrentLimit>) = windows(dashboard)
    .into_iter()
    .partition(|window| WindowKind::of(window.minutes) == WindowKind::FiveHour);
  let undrawn = dashboard.grouping == Grouping::Day;
  Row::new()
    .id("usage-limits-legend")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(column(ctx, dashboard, &short, undrawn))
    .child(column(ctx, dashboard, &long, false))
    .into()
}

fn column(ctx: &Ctx, dashboard: &Dashboard, windows: &[&CurrentLimit], faded: bool) -> Element {
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .with_children(windows.iter().map(|window| entry(ctx, dashboard, window, faded)))
    .into()
}

/// One window: its line, name, share and reset.
fn entry(ctx: &Ctx, dashboard: &Dashboard, window: &CurrentLimit, faded: bool) -> Element {
  let (value, resets) = reading(ctx, dashboard, window);
  let id = format!(
    "usage-limits-window-{}-{}",
    CliSeries::of(Some(window.provider)).index(),
    window.minutes
  );
  let line = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(line_swatch(
      &series_color(CliSeries::of(Some(window.provider))),
      WindowKind::of(window.minutes) == WindowKind::FiveHour,
    ))
    .child(
      Text::new(&line_name(ctx, window.provider, window.minutes))
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .child(
      Text::new(&value)
        .id(format!("{id}-value"))
        .variant(TypographyStyle::Mono)
        .nowrap(),
    );
  let entry = Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(line)
    .with_children(resets.map(|resets| {
      Row::new()
        .padding(Padding::new().left(UsageSpacing::ResetsIndent.role()))
        .child(
          Text::new(&resets)
            .id(format!("{id}-resets"))
            .variant(TypographyStyle::Caption)
            .nowrap(),
        )
    }));
  if faded { entry.opacity(UNDRAWN_OPACITY) } else { entry }.into()
}

/// A window's line in a legend: solid for a weekly window, dashed for a 5-hour one.
fn line_swatch(color: &PaletteColor, dashed: bool) -> Element {
  let pieces = if dashed { DASHES } else { 1 };
  Row::new()
    .width(Dimension::Px(LINE_WIDTH))
    .height(Dimension::Px(LINE_ROOM))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .with_children((0..pieces).map(|_| {
      Row::new()
        .flex(1.0)
        .height(Dimension::Px(LINE_HEIGHT))
        .background(color.clone())
        .rounded(UsageRadius::Bar.role())
    }))
    .into()
}

/// The share last reported and when the window resets; once the reset has passed the share is no longer
/// known until a CLI reports again.
fn reading(ctx: &Ctx, dashboard: &Dashboard, window: &CurrentLimit) -> (Arc<str>, Option<Arc<str>>) {
  let share = words::share(ctx, window.percent.map(|percent| percent.round() as u64));
  match window.resets {
    Some(resets) if resets <= dashboard.now => (
      ctx.t("usage.unknown"),
      Some(ctx.t_args(
        "usage.limits.wasReset",
        [("time", &*words::reset_time(ctx, resets, dashboard.now))],
      )),
    ),
    Some(resets) => (
      share,
      Some(ctx.t_args(
        "usage.limits.resets",
        [("time", &*words::reset_time(ctx, resets, dashboard.now))],
      )),
    ),
    None => (share, None),
  }
}

/// The card without its chart: the windows last reported and when; `menu` keeps the warning threshold
/// within reach.
pub fn compact(ctx: &Ctx, dashboard: &Dashboard, menu: Element) -> Element {
  let card = Card::new("usage-limits", ctx.t("usage.limits.title"))
    .note(ctx.t("usage.limits.note"))
    .trailing(menu);
  let Some(last) = dashboard.limits.current.iter().map(|window| window.at).max() else {
    return card.body([footnote("usage-limits-caption", &ctx.t("usage.limits.noneYet"))]);
  };
  let reported = words::reported(ctx, last, dashboard.now);
  let filtered =
    matches!(dashboard.state, DashboardState::FilteredOut { .. }) || !dashboard.query.filter.clis.is_empty();
  let caption = if filtered {
    ctx.t_args("usage.limits.cliFilterOnly", [("time", &*reported)])
  } else {
    ctx.t_args("usage.limits.lastReported", [("time", &*reported)])
  };
  // Windows that reset since their last report say nothing more here, unless nothing else is known.
  let all = windows(dashboard);
  let live: Vec<&CurrentLimit> = all
    .iter()
    .copied()
    .filter(|window| window.resets.is_none_or(|resets| resets > dashboard.now))
    .collect();
  let shown = if live.is_empty() { all } else { live };
  let (short, long): (Vec<&CurrentLimit>, Vec<&CurrentLimit>) = shown
    .into_iter()
    .partition(|window| WindowKind::of(window.minutes) == WindowKind::FiveHour);
  let columns: Vec<Element> = if short.is_empty() {
    long
      .chunks(long.len().div_ceil(2).max(1))
      .map(|half| column(ctx, dashboard, half, false))
      .collect()
  } else {
    vec![
      column(ctx, dashboard, &short, false),
      column(ctx, dashboard, &long, false),
    ]
  };
  let entries = Row::new()
    .id("usage-limits-legend")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .with_children(columns);
  card.body([entries.into(), footnote("usage-limits-caption", &caption)])
}
