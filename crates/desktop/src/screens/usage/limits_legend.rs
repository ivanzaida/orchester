//! What *Plan limits* says of each window (`C2pTYF`, `e5PbG`): a dashed line for a 5-hour window, a
//! solid one for a weekly window, its name, the share last reported and when it resets
//! (*Codex · weekly 41% resets Oct 4, 09:00*). Above the chart the 5-hour windows and the weekly
//! ones each take a column; grouped by day, 5-hour windows are not drawn and their entries are
//! dimmed but keep the value. Without runs to chart, or when the filters match nothing, the card
//! shrinks to the windows alone (`AY6FM`, `YxAed`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{
  CliSeries, Grouping,
  limits::{CurrentLimit, WindowKind},
};

use super::{
  card::{Card, line_swatch, series_color},
  format,
  limits_chart::window_name,
  state::Dashboard,
};
use crate::theme::{ExtraSpacing, ExtraTypography};

/// The compact card's entries take three times the title's share of its header.
const ENTRIES_SHARE: f32 = 3.0;

/// The windows in legend order: 5-hour windows first, each kind by CLI.
fn windows(dashboard: &Dashboard) -> Vec<&CurrentLimit> {
  let mut windows: Vec<&CurrentLimit> = dashboard.limits.current.iter().collect();
  windows.sort_by_key(|window| {
    (
      WindowKind::of(window.minutes) != WindowKind::FiveHour,
      CliSeries::of(Some(window.provider)),
      window.minutes,
    )
  });
  windows
}

/// The legend above the chart: 5-hour windows in the first column, the others in the second.
pub fn legend(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let (short, long): (Vec<&CurrentLimit>, Vec<&CurrentLimit>) = windows(dashboard)
    .into_iter()
    .partition(|window| WindowKind::of(window.minutes) == WindowKind::FiveHour);
  let column = |windows: Vec<&CurrentLimit>| {
    Element::from(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .with_children(windows.into_iter().map(|window| {
          let undrawn = dashboard.grouping == Grouping::Day && WindowKind::of(window.minutes) == WindowKind::FiveHour;
          entry(ctx, dashboard, window, undrawn)
        })),
    )
  };
  Row::new()
    .id("usage-limits-legend")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .with_children([column(short), column(long)])
    .into()
}

/// One window: its line, name, share and reset; `dimmed` when the chart does not draw it.
fn entry(ctx: &Ctx, dashboard: &Dashboard, window: &CurrentLimit, dimmed: bool) -> Element {
  let kind = WindowKind::of(window.minutes);
  let (name_color, value_color, note_color) = if dimmed {
    (
      PaletteColor::TextMuted,
      PaletteColor::TextMuted,
      PaletteColor::TextMuted,
    )
  } else {
    (
      PaletteColor::TextPrimary,
      PaletteColor::TextPrimary,
      PaletteColor::TextSecondary,
    )
  };
  let name = ctx.t_args(
    "usage.limits.legendName",
    [
      ("cli", &*format::cli_name(ctx, window.provider)),
      ("window", &*window_name(ctx, window.minutes)),
    ],
  );
  let (value, note) = reading(ctx, dashboard, window);
  Row::new()
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .child(line_swatch(
      &series_color(CliSeries::of(Some(window.provider))),
      kind == WindowKind::FiveHour,
    ))
    .child(
      Text::new(&name)
        .variant(TypographyStyle::Caption)
        .color(name_color)
        .nowrap(),
    )
    .child(
      Text::new(&value)
        .variant(ExtraTypography::MonoValue.role())
        .color(value_color)
        .nowrap(),
    )
    .with_children(note.map(|note| {
      Element::from(
        Text::new(&note)
          .variant(TypographyStyle::Caption)
          .color(note_color)
          .nowrap(),
      )
    }))
    .into()
}

/// The share last reported and when the window resets; once the reset has passed the share is no
/// longer known until a CLI reports again.
fn reading(ctx: &Ctx, dashboard: &Dashboard, window: &CurrentLimit) -> (Arc<str>, Option<Arc<str>>) {
  let share = format::share(ctx, window.percent.map(|percent| percent.round() as u64));
  match window.resets {
    Some(resets) if resets <= dashboard.now => (
      ctx.t("usage.unknown"),
      Some(ctx.t_args(
        "usage.limits.wasReset",
        [("time", &*format::reset_time(ctx, resets, dashboard.now))],
      )),
    ),
    Some(resets) => (
      share,
      Some(ctx.t_args(
        "usage.limits.resets",
        [("time", &*format::reset_time(ctx, resets, dashboard.now))],
      )),
    ),
    None => (share, None),
  }
}

/// The card without its chart: the windows last reported, and when (`AY6FM`, `YxAed`); `menu` keeps
/// the warning threshold within reach.
pub fn compact(ctx: &Ctx, dashboard: &Dashboard, menu: Element) -> Element {
  let card = Card::new("usage-limits", ctx.t("usage.limits.title"));
  let Some(last) = dashboard.limits.current.iter().map(|window| window.at).max() else {
    return card.subtitle(ctx.t("usage.limits.none")).trailing(menu).body([]);
  };
  let reported = format::reported(ctx, last, dashboard.now);
  let subtitle = if dashboard.filter.clis.is_empty() && !dashboard.filtered_out {
    ctx.t_args("usage.limits.lastReported", [("time", &*reported)])
  } else {
    ctx.t_args("usage.limits.cliFilterOnly", [("time", &*reported)])
  };
  // Windows that reset since their last report say nothing more here, unless nothing else is known.
  let all = windows(dashboard);
  let live: Vec<&CurrentLimit> = all
    .iter()
    .copied()
    .filter(|window| window.resets.is_none_or(|resets| resets > dashboard.now))
    .collect();
  let shown = if live.is_empty() { all } else { live };
  // Most of the header's width: the title keeps a quarter, and on a narrow window what does not fit
  // wraps under the first entries.
  let entries = Row::new()
    .flex(ENTRIES_SHARE)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .justify(Justify::End)
    .wrap()
    .with_children(shown.into_iter().map(|window| entry(ctx, dashboard, window, false)));
  card.subtitle(subtitle).trailing(entries.into()).trailing(menu).body([])
}
