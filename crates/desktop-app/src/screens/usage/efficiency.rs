//! *Efficiency* (`Efficiency` `vVqe4`): a heading over three cards, each with its own chart / table glyphs.
//! *Cache share* (`a8dQJu`): the period's share against the period before and one line over the days.
//! *Tokens before first edit* (`M94B7Y`): the median per invocation that changed a file, with and without
//! the code index, as single-series bars with their invocation counts; invocations without an edit are left
//! out and counted. *Failed runs* is [`super::failed`].

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{Change, EditTokens};
use orchester_workspace::usage::Dashboard;

use super::{
  Requery, Views,
  card::{Card, footnote, push_down, view_switch},
  failed,
  line_draw::{CacheLine, Day, LineProps},
  plot::{AxisName, label_every},
  table::{Cell, Density, Width, column, row, table},
  words,
};
use crate::theme::{ExtraSpacing, UsageRadius, UsageText};

/// Day labels a cache share line keeps, at most.
const DAY_LABELS: usize = 7;
/// A bar of *Tokens before first edit* (`Bar · With code index` `oVuBm`).
const BAR_HEIGHT: f32 = 8.0;
/// Table columns (`tW06R`, `oDlsA`).
const SHARE_COLUMN: f32 = 88.0;
const MEDIAN_COLUMN: f32 = 64.0;
const COUNT_COLUMN: f32 = 84.0;

pub fn section(ctx: &mut Ctx, dashboard: &Arc<Dashboard>, views: &Signal<Views>, requery: &Requery) -> Element {
  let cards = vec![
    cache(ctx, dashboard, views),
    edits(ctx, dashboard, views),
    failed::card(ctx, dashboard, views, requery),
  ];
  Column::new()
    .id("usage-efficiency")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Text::new(&ctx.t("usage.efficiency.title"))
        .id("usage-efficiency-title")
        .variant(TypographyStyle::Title),
    )
    .child(super::card::card_row("usage-efficiency-cards", cards))
    .into()
}

/// A card's chart / table glyphs, flipping `field` of the views.
pub fn switch(ctx: &Ctx, id: &str, views: &Signal<Views>, field: fn(&mut Views) -> &mut bool) -> Element {
  let table = *field(&mut views.get());
  let views = views.clone();
  view_switch(ctx, id, table, true, move |table| {
    views.update(|views| *field(views) = table);
  })
}

fn cache(ctx: &mut Ctx, dashboard: &Dashboard, views: &Signal<Views>) -> Element {
  let card = Card::new("usage-cache", ctx.t("usage.efficiency.cache"))
    .small()
    .subtitle(ctx.t("usage.efficiency.cacheSubtitle"))
    .trailing(switch(ctx, "usage-cache-view", views, |views| &mut views.cache_table))
    .fill();
  if views.get().cache_table {
    return card.body([cache_table(ctx, dashboard)]);
  }
  card.body([cache_figure(ctx, dashboard), cache_line(ctx, dashboard)])
}

/// *71%* and *in the period · up from 68%* (`Figure` `gImvs`).
fn cache_figure(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let (now, before) = (
    dashboard.report.summary.cache_share(),
    dashboard.report.previous.cache_share(),
  );
  let before_text = before.map(|before| words::share(ctx, Some(before)));
  let caption = match (Change::points(now, before), before_text) {
    (Some(Change::More(_)), Some(before)) => ctx.t_args("usage.efficiency.cameUp", [("before", &*before)]),
    (Some(Change::Less(_)), Some(before)) => ctx.t_args("usage.efficiency.cameDown", [("before", &*before)]),
    (Some(Change::Same), _) => ctx.t("usage.efficiency.sameAsBefore"),
    _ => ctx.t("usage.efficiency.inPeriod"),
  };
  Row::new()
    .id("usage-cache-figure")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::End)
    .child(
      Text::new(&words::share(ctx, now))
        .variant(UsageText::Figure.role())
        .nowrap(),
    )
    .child(
      Text::new(&caption)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

fn cache_line(ctx: &mut Ctx, dashboard: &Dashboard) -> Element {
  let days = &dashboard.report.efficiency.cache_by_day;
  let every = label_every(days.len(), DAY_LABELS as f32, 1.0);
  let (last, today) = (days.len().saturating_sub(1), dashboard.today());
  // Counted back from the last day, so it always has its label, as the other charts do.
  let days: Arc<[Day]> = days
    .iter()
    .enumerate()
    .map(|(index, (date, share))| Day {
      label: (last - index)
        .is_multiple_of(every)
        .then(|| short_day(ctx, *date, today)),
      share: *share,
      name: words::weekday(ctx, *date),
      value: words::share(ctx, *share),
    })
    .collect();
  let percent = |value: u64| words::share(ctx, Some(value));
  ctx.mount_keyed::<CacheLine>(
    "usage-cache-line",
    LineProps {
      days,
      percents: [percent(0), percent(50), percent(100)],
    },
  )
}

/// A day under the cache share line: its weekday alone (`Wed`), or *Today*.
fn short_day(ctx: &Ctx, date: chrono::NaiveDate, today: chrono::NaiveDate) -> AxisName {
  if date == today {
    AxisName::day(ctx, date, today)
  } else {
    AxisName::plain(words::weekday_short(ctx, date))
  }
}

fn cache_table(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let columns = [
    column(ctx.t("usage.table.day"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.cacheShare"), Width::Px(SHARE_COLUMN), true),
  ];
  let days = &dashboard.report.efficiency.cache_by_day;
  let mut rows: Vec<Element> = days
    .iter()
    .enumerate()
    .map(|(index, (date, share))| {
      let name = if *date == dashboard.today() {
        ctx.t_args(
          "usage.tokens.todayUntil",
          [("time", orchester_workspace::display::clock(dashboard.now).as_str())],
        )
      } else {
        words::long_day(ctx, *date)
      };
      let cells = vec![Cell::Text(name), Cell::Text(words::share(ctx, *share))];
      row(
        &format!("usage-cache-table-row-{index}"),
        &columns,
        Density::Data,
        cells,
      )
      .into()
    })
    .collect();
  let total = vec![
    Cell::Strong(ctx.t("usage.table.total")),
    Cell::Strong(words::share(ctx, dashboard.report.summary.cache_share())),
  ];
  rows.push(row("usage-cache-table-total", &columns, Density::Data, total).into());
  table("usage-cache-table", &columns, Density::Data, rows)
}

fn edits(ctx: &mut Ctx, dashboard: &Dashboard, views: &Signal<Views>) -> Element {
  let card = Card::new("usage-edits", ctx.t("usage.efficiency.beforeEdit"))
    .small()
    .subtitle(ctx.t("usage.efficiency.beforeEditSubtitle"))
    .trailing(switch(ctx, "usage-edits-view", views, |views| &mut views.edits_table))
    .fill();
  let efficiency = &dashboard.report.efficiency;
  let without_edit = efficiency.with_index.without_edit + efficiency.without_index.without_edit;
  let note = footnote(
    "usage-edits-footnote",
    &ctx.t_args(
      "usage.efficiency.beforeEditNote",
      [("count", without_edit.to_string().as_str())],
    ),
  );
  if views.get().edits_table {
    return card.body([edits_table(ctx, dashboard), push_down(), note]);
  }
  let largest = efficiency
    .with_index
    .median
    .max(efficiency.without_index.median)
    .unwrap_or(0)
    .max(1);
  let bars = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inset.role())
    .child(edit_bar(
      ctx,
      ("usage-edits-with", "usage.efficiency.withIndex"),
      efficiency.with_index,
      largest,
    ))
    .child(edit_bar(
      ctx,
      ("usage-edits-without", "usage.efficiency.withoutIndex"),
      efficiency.without_index,
      largest,
    ));
  card.body([bars.into(), push_down(), note])
}

fn edit_bar(ctx: &Ctx, (id, key): (&str, &str), tokens: EditTokens, largest: u64) -> Element {
  let value = tokens.median.map_or_else(|| ctx.t("usage.unknown"), words::tokens);
  let width = tokens
    .median
    .map_or(0.0, |median| median as f32 / largest as f32 * 100.0);
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Pair.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Description).nowrap())
        .child(Spacer::new().flex(1.0))
        .child(
          Text::new(&value)
            .id(format!("{id}-median"))
            .variant(TypographyStyle::Mono)
            .nowrap(),
        ),
    )
    .child(bar(width))
    .child(
      Text::new(&ctx.t_args(
        "usage.efficiency.invocations",
        [("count", tokens.edited.to_string().as_str())],
      ))
      .variant(TypographyStyle::Caption)
      .nowrap(),
    )
    .into()
}

/// A single-series bar `width` percent of its row wide, in `text_muted`.
fn bar(width: f32) -> Element {
  Row::new()
    .width(Dimension::Pct(width))
    .height(Dimension::Px(BAR_HEIGHT))
    .background(PaletteColor::TextMuted)
    .rounded(UsageRadius::Bar.role())
    .into()
}

fn edits_table(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let efficiency = &dashboard.report.efficiency;
  let columns = [
    column(ctx.t("usage.table.name"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.median"), Width::Px(MEDIAN_COLUMN), true),
    column(ctx.t("usage.table.invocations"), Width::Px(COUNT_COLUMN), true),
  ];
  let rows = [
    ("with", "usage.efficiency.withIndex", efficiency.with_index),
    ("without", "usage.efficiency.withoutIndex", efficiency.without_index),
  ]
  .into_iter()
  .map(|(id, key, tokens)| {
    let median = tokens
      .median
      .map_or_else(|| ctx.t("usage.unknown"), |median| words::exact(ctx, median));
    let cells = vec![
      Cell::Text(ctx.t(key)),
      Cell::Text(median),
      Cell::Text(tokens.edited.to_string().into()),
    ];
    row(&format!("usage-edits-table-{id}"), &columns, Density::Data, cells).into()
  })
  .collect();
  table("usage-edits-table", &columns, Density::Data, rows)
}
