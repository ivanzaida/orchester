//! *Efficiency* (`C2pTYF`): a section heading over three cards, each with its own chart / table
//! switch. *Cache share*: the period's share against the previous one and one line per day.
//! *Tokens before first edit*: the median per invocation that changed a file, with and without the
//! code index (single-series bars in `$subtle` with invocation counts; invocations without an edit
//! are left out and counted). *Failed runs* is in [`super::failed`].

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
use orchester_core::usage::{Change, EditTokens, local_date};

use super::{
  card::{Card, card_row, footnote, push_down, view_switch},
  failed, format,
  line_draw::{CacheLine, Day, LineProps},
  plot::{AxisName, label_every},
  state::{Dashboard, Views},
  table::{Cell, Width, column, data_row, table},
};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, chart::BREAKDOWN_BAR_HEIGHT},
  ui::icon::Icon,
};

/// Day labels a cache share line keeps, at most.
const DAY_LABELS: usize = 7;

pub fn efficiency(ctx: &mut Ctx, dashboard: &Dashboard, views: &Signal<Views>) -> Element {
  let cards = vec![
    cache(ctx, dashboard, views),
    edits(ctx, dashboard, views),
    failed::failed(ctx, dashboard, views),
  ];
  Column::new()
    .id("usage-efficiency")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(Text::new(&ctx.t("usage.efficiency.title")).variant(ExtraTypography::SectionHeading.role()))
    .child(card_row(ctx, cards))
    .into()
}

/// A card's *Chart* / *Table* glyphs, flipping `field` of the views.
pub fn switch(
  ctx: &Ctx,
  (id, chart): (&'static str, Icon),
  views: &Signal<Views>,
  field: fn(&mut Views) -> &mut bool,
) -> Element {
  let table = *field(&mut views.get());
  let views = views.clone();
  view_switch(ctx, (id, chart), table, true, move |table| {
    views.update(|views| *field(views) = table);
  })
}

fn cache(ctx: &mut Ctx, dashboard: &Dashboard, views: &Signal<Views>) -> Element {
  let card = Card::new("usage-cache", ctx.t("usage.efficiency.cache"))
    .subtitle(ctx.t("usage.efficiency.cacheSubtitle"))
    .trailing(switch(ctx, ("usage-cache-view", Icon::ChartColumn), views, |views| {
      &mut views.cache_table
    }))
    .fill();
  if views.get().cache_table {
    return card.body([cache_table(ctx, dashboard)]);
  }
  card.body([cache_figure(ctx, dashboard), cache_line(ctx, dashboard)])
}

/// *71%* and *in the period · up from 67%*.
fn cache_figure(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let (now, before) = (
    dashboard.report.summary.cache_share(),
    dashboard.report.previous.cache_share(),
  );
  let before_text = before.map(|before| format::share(ctx, Some(before)));
  let caption = match (Change::points(now, before), before_text) {
    (Some(Change::More(_)), Some(before)) => ctx.t_args("usage.efficiency.cameUp", [("before", &*before)]),
    (Some(Change::Less(_)), Some(before)) => ctx.t_args("usage.efficiency.cameDown", [("before", &*before)]),
    (Some(Change::Same), _) => ctx.t("usage.efficiency.sameAsBefore"),
    _ => ctx.t("usage.efficiency.inPeriod"),
  };
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::End)
    .child(
      Text::new(&format::share(ctx, now))
        .variant(ExtraTypography::Metric.role())
        .nowrap(),
    )
    .child(Text::new(&caption).variant(TypographyStyle::Caption))
    .into()
}

fn cache_line(ctx: &mut Ctx, dashboard: &Dashboard) -> Element {
  let days = &dashboard.report.efficiency.cache_by_day;
  let every = label_every(days.len(), DAY_LABELS as f32, 1.0);
  let (last, today) = (days.len().saturating_sub(1), local_date(dashboard.now));
  // Counted back from the last day, so it always has its label, as the other charts do.
  let days: Arc<[Day]> = days
    .iter()
    .enumerate()
    .map(|(index, (date, share))| Day {
      label: (last - index)
        .is_multiple_of(every)
        .then(|| AxisName::day(ctx, *date, today)),
      share: *share,
      name: format::weekday(ctx, *date),
      value: format::share(ctx, *share),
    })
    .collect();
  let percent = |value: u64| format::share(ctx, Some(value));
  ctx.mount_keyed::<CacheLine>(
    "usage-cache-line",
    LineProps {
      days,
      percents: [percent(0), percent(25), percent(50), percent(75), percent(100)],
    },
  )
}

fn cache_table(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let columns = [
    column(ctx.t("usage.table.day"), Width::Flex(1.4), false),
    column(ctx.t("usage.efficiency.cache"), Width::Flex(1.0), true),
  ];
  let rows = dashboard
    .report
    .efficiency
    .cache_by_day
    .iter()
    .map(|(date, share)| {
      data_row(
        &columns,
        vec![
          Cell::Text(format::long_day(ctx, *date)),
          Cell::Text(format::share(ctx, *share)),
        ],
      )
    })
    .collect();
  table("usage-cache-table", &columns, rows)
}

fn edits(ctx: &Ctx, dashboard: &Dashboard, views: &Signal<Views>) -> Element {
  let card = Card::new("usage-edits", ctx.t("usage.efficiency.beforeEdit"))
    .subtitle(ctx.t("usage.efficiency.beforeEditSubtitle"))
    .trailing(switch(ctx, ("usage-edits-view", Icon::ChartColumn), views, |views| {
      &mut views.edits_table
    }))
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
  card.body([
    edit_bar(ctx, "usage.efficiency.withIndex", efficiency.with_index, largest),
    edit_bar(ctx, "usage.efficiency.withoutIndex", efficiency.without_index, largest),
    push_down(),
    note,
  ])
}

fn edit_bar(ctx: &Ctx, key: &str, tokens: EditTokens, largest: u64) -> Element {
  let value = tokens.median.map_or_else(|| ctx.t("usage.unknown"), format::tokens);
  let width = tokens
    .median
    .map_or(0.0, |median| median as f32 / largest as f32 * 100.0);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Body).nowrap())
        .child(Spacer::new().flex(1.0))
        .child(Text::new(&value).variant(ExtraTypography::MonoValue.role()).nowrap())
        .child(
          Text::new(&ctx.t_args(
            "usage.efficiency.invocations",
            [("count", tokens.edited.to_string().as_str())],
          ))
          .variant(TypographyStyle::Caption)
          .nowrap(),
        ),
    )
    .child(bar(width, PaletteColor::TextMuted))
    .into()
}

/// A single-series bar `width` percent of its row wide.
pub fn bar(width: f32, color: PaletteColor) -> Element {
  Row::new()
    .width(Dimension::Pct(width))
    .height(Dimension::Px(BREAKDOWN_BAR_HEIGHT))
    .background(color)
    .rounded(ExtraRadius::Mark.role())
    .into()
}

fn edits_table(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let efficiency = &dashboard.report.efficiency;
  let columns = [
    column(ctx.t("usage.table.name"), Width::Flex(1.4), false),
    column(ctx.t("usage.table.median"), Width::Flex(1.0), true),
    column(ctx.t("usage.table.invocations"), Width::Flex(0.8), true),
  ];
  let rows = [
    ("usage.efficiency.withIndex", efficiency.with_index),
    ("usage.efficiency.withoutIndex", efficiency.without_index),
  ]
  .into_iter()
  .map(|(key, tokens)| {
    let median = tokens
      .median
      .map_or_else(|| ctx.t("usage.unknown"), |median| format::exact(ctx, median));
    data_row(
      &columns,
      vec![
        Cell::Text(ctx.t(key)),
        Cell::Text(median),
        Cell::Text(tokens.edited.to_string().into()),
      ],
    )
  })
  .collect();
  table("usage-edits-table", &columns, rows)
}
