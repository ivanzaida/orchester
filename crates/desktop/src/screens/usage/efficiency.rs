//! *Efficiency* (`C2pTYF`): the cache share per day; *Tokens before first edit*, the median per
//! invocation that changed a file, with and without the code index (single-series bars in
//! `$subtle` with invocation counts; invocations without an edit are left out and counted); and
//! *Failed runs* — count and share, the tokens spent on them and their share of spent as a bar, the
//! largest failed run and *Show failed tasks →*, which opens the Overview's *Failed* filter.

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
use orchester_core::usage::{EditTokens, percent};

use super::{
  card::{Card, view_switch},
  format,
  line_draw::{CacheLine, Day, LineProps},
  load::Scope,
  plot::label_every,
  state::{Dashboard, Views},
  table::{Cell, Width, column, row, table},
};
use crate::{
  navigation::route::Page,
  screens::project::ProjectNav,
  tasks::overview::OverviewTab,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, chart::BREAKDOWN_BAR_HEIGHT},
  ui::button::ActionButton,
};

/// Day labels a cache share line keeps, at most.
const DAY_LABELS: usize = 7;

pub fn efficiency(ctx: &mut Ctx, dashboard: &Dashboard, views: &Signal<Views>) -> Element {
  let table_shown = views.get().efficiency_table;
  let views = views.clone();
  let switch = view_switch(ctx, "usage-efficiency-view", table_shown, move |table| {
    views.update(|views| views.efficiency_table = table);
  });
  let card = Card::new("usage-efficiency", ctx.t("usage.efficiency.title")).trailing(switch);
  if table_shown {
    return card.body([efficiency_table(ctx, dashboard)]);
  }
  let cache = cache_line(ctx, dashboard);
  let nav = ctx.use_context::<ProjectNav>();
  let columns = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(section(ctx, "usage.efficiency.cache", vec![cache]))
    .child(section(ctx, "usage.efficiency.beforeEdit", edits(ctx, dashboard)))
    .child(section(ctx, "usage.efficiency.failed", failed(ctx, dashboard, nav)));
  card.body([columns.into()])
}

fn section(ctx: &Ctx, key: &str, children: Vec<Element>) -> Element {
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Label))
    .with_children(children)
    .into()
}

fn cache_line(ctx: &mut Ctx, dashboard: &Dashboard) -> Element {
  let days = &dashboard.report.efficiency.cache_by_day;
  let every = label_every(days.len(), DAY_LABELS as f32, 1.0);
  let days: Arc<[Day]> = days
    .iter()
    .enumerate()
    .map(|(index, (date, share))| Day {
      label: (index % every == 0).then(|| format::weekday(ctx, *date)),
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

/// With and without the code index, each a bar scaled to the larger median.
fn edits(ctx: &Ctx, dashboard: &Dashboard) -> Vec<Element> {
  let efficiency = &dashboard.report.efficiency;
  let largest = efficiency
    .with_index
    .median
    .max(efficiency.without_index.median)
    .unwrap_or(0)
    .max(1);
  let without_edit = efficiency.with_index.without_edit + efficiency.without_index.without_edit;
  vec![
    edit_bar(ctx, "usage.efficiency.withIndex", efficiency.with_index, largest),
    edit_bar(ctx, "usage.efficiency.withoutIndex", efficiency.without_index, largest),
    Text::new(&ctx.t_args(
      "usage.efficiency.withoutEdit",
      [("count", without_edit.to_string().as_str())],
    ))
    .variant(TypographyStyle::Caption)
    .color(PaletteColor::TextMuted)
    .into(),
  ]
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
        .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption).nowrap())
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

fn bar(width: f32, color: PaletteColor) -> Element {
  Row::new()
    .width(Dimension::Pct(width))
    .height(Dimension::Px(BREAKDOWN_BAR_HEIGHT))
    .background(color)
    .rounded(ExtraRadius::Mark.role())
    .into()
}

fn failed(ctx: &Ctx, dashboard: &Dashboard, nav: Option<ProjectNav>) -> Vec<Element> {
  let failed = &dashboard.report.efficiency.failed;
  let share = percent(failed.spent, failed.of_spent);
  let mut children: Vec<Element> = vec![
    Text::new(&ctx.t_args(
      "usage.efficiency.failedRuns",
      [
        ("count", failed.runs.to_string().as_str()),
        (
          "share",
          &*format::share(ctx, percent(failed.runs as u64, failed.of_runs as u64)),
        ),
      ],
    ))
    .variant(TypographyStyle::Body)
    .into(),
    Text::new(&ctx.t_args(
      "usage.efficiency.failedTokens",
      [
        ("tokens", &*format::tokens(failed.spent)),
        ("share", &*format::share(ctx, share)),
      ],
    ))
    .variant(TypographyStyle::Caption)
    .into(),
    bar(share.unwrap_or(0) as f32, PaletteColor::TextMuted),
  ];
  if let Some(largest) = &failed.largest {
    children.push(
      Text::new(&ctx.t_args(
        "usage.efficiency.largest",
        [
          ("task", &*largest.key),
          ("run", largest.number.get().to_string().as_str()),
          ("tokens", &*format::tokens(largest.usage.counted())),
        ],
      ))
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextMuted)
      .into(),
    );
  }
  if let (Scope::Project(_), Some(nav), true) = (&dashboard.scope, nav, failed.runs > 0) {
    children.push(
      ActionButton::new(ctx.t("usage.efficiency.showFailed"))
        .id("usage-show-failed")
        .compact()
        .link(nav.link(Page::Overview(OverviewTab::Failed)))
        .render(ctx),
    );
  }
  children
}

/// The section's figures with exact numbers.
fn efficiency_table(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let efficiency = &dashboard.report.efficiency;
  let columns = [
    column(ctx.t("usage.table.measure"), Width::Flex(1.6), false),
    column(ctx.t("usage.table.value"), Width::Flex(1.0), true),
    column(ctx.t("usage.table.count"), Width::Flex(0.8), true),
  ];
  let exact = |value: Option<u64>| value.map_or_else(|| ctx.t("usage.unknown"), |value| format::exact(ctx, value));
  let mut rows: Vec<Row> = efficiency
    .cache_by_day
    .iter()
    .map(|(date, share)| {
      let name = ctx.t_args("usage.efficiency.cacheOn", [("day", &*format::weekday(ctx, *date))]);
      row(
        &columns,
        vec![
          Cell::Text(name),
          Cell::Text(format::share(ctx, *share)),
          Cell::Text(Arc::from("")),
        ],
      )
    })
    .collect();
  for (key, tokens) in [
    ("usage.efficiency.withIndex", efficiency.with_index),
    ("usage.efficiency.withoutIndex", efficiency.without_index),
  ] {
    rows.push(row(
      &columns,
      vec![
        Cell::Text(ctx.t(key)),
        Cell::Text(exact(tokens.median)),
        Cell::Text(tokens.edited.to_string().into()),
      ],
    ));
  }
  let failed = &efficiency.failed;
  rows.push(row(
    &columns,
    vec![
      Cell::Text(ctx.t("usage.efficiency.failed")),
      Cell::Text(format::exact(ctx, failed.spent)),
      Cell::Text(failed.runs.to_string().into()),
    ],
  ));
  table("usage-efficiency-table", &columns, rows)
}
