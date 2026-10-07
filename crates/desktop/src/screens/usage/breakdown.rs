//! *Where tokens go* (`C2pTYF`, `p4eVV6`): columns of horizontal bars, stacked by CLI and scaled to
//! the column's largest row, each with its spent tokens and share — *By agent profile · step*, *By
//! model* (mono) and *By pipeline*; across projects *By project* comes first and names the projects
//! without runs. Selecting a row adds it as a filter. Auto mode's reviewer has rows of its own among the
//! profiles (*Auto mode reviewer · claude-haiku-4-5*); it is no agent profile, so they only read.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension},
};
use orchester_core::usage::{BreakdownKey, BreakdownRow, CliSeries, percent, toggle};

use super::{
  card::{Card, legend_item, series_color, view_switch},
  format,
  state::{Controls, Dashboard, Views},
  table::{Cell, Width, column, data_row, table},
};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, chart::BREAKDOWN_BAR_HEIGHT},
  ui::{
    icon::Icon,
    pressable::{pressable, surface},
  },
};

/// Rows a column shows; the rest are in the table.
const ROWS: usize = 6;

pub fn breakdown(ctx: &Ctx, dashboard: &Dashboard, views: &Signal<Views>, controls: &Signal<Controls>) -> Element {
  let table_shown = views.get().breakdown_table;
  let views = views.clone();
  let switch = view_switch(
    ctx,
    ("usage-breakdown-view", Icon::ChartColumn),
    table_shown,
    false,
    move |table| views.update(|views| views.breakdown_table = table),
  );
  let breakdowns = &dashboard.report.breakdowns;
  let mut columns = Vec::new();
  if dashboard.all_projects() {
    columns.push(("usage.breakdown.byProject", &breakdowns.projects));
  }
  columns.extend([
    ("usage.breakdown.byProfile", &breakdowns.profiles),
    ("usage.breakdown.byModel", &breakdowns.models),
    ("usage.breakdown.byPipeline", &breakdowns.pipelines),
  ]);
  let spent = dashboard.report.summary.spent();
  let body = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .with_children(columns.into_iter().map(|(key, rows)| {
      let mut column = Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Sm)
        .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Label));
      column = if table_shown {
        column.child(rows_table(ctx, rows, spent))
      } else {
        column.with_children(
          rows
            .iter()
            .take(ROWS)
            .map(|row| bar_row(ctx, row, rows, spent, controls)),
        )
      };
      if key == "usage.breakdown.byProject" && !breakdowns.idle_projects.is_empty() {
        column = column.child(
          Text::new(&ctx.t_args(
            "usage.breakdown.idle",
            [(
              "names",
              breakdowns.idle_projects.join(&ctx.t("usage.listSeparator")).as_str(),
            )],
          ))
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextMuted),
        );
      }
      Element::from(column)
    }));
  Card::new("usage-breakdown", ctx.t("usage.breakdown.title"))
    .lead(series_legend(ctx, dashboard))
    .note(ctx.t("usage.breakdown.note"))
    .trailing(switch)
    .body([body.into()])
}

/// The CLIs the bars are split by, each with its swatch.
fn series_legend(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let rows = &dashboard.report.breakdowns.pipelines;
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(
      CliSeries::ALL
        .into_iter()
        .filter(|series| rows.iter().any(|row| row.by_cli[series.index()] > 0))
        .map(|series| legend_item(series_color(series), &format::series_name(ctx, series), None)),
    )
    .into()
}

/// A row's name: *Builder · implement*, *Auto mode reviewer · claude-haiku-4-5*, a model in mono, a
/// pipeline or a project.
fn label(ctx: &Ctx, key: &BreakdownKey) -> (Arc<str>, bool) {
  match key {
    BreakdownKey::ProfileStep { profile, step } => (
      ctx.t_args(
        "usage.breakdown.profileStep",
        [("profile", &**profile), ("step", &**step)],
      ),
      false,
    ),
    BreakdownKey::Reviewer(model) => (ctx.t_args("usage.breakdown.reviewer", [("model", &**model)]), false),
    BreakdownKey::Model(model) => (model.clone(), true),
    BreakdownKey::Pipeline(Some(pipeline)) => (pipeline.clone(), false),
    BreakdownKey::Pipeline(None) => (ctx.t("usage.breakdown.pipelineGone"), false),
    BreakdownKey::Project(_, name) => (name.clone(), false),
  }
}

fn bar_row(ctx: &Ctx, row: &BreakdownRow, rows: &[BreakdownRow], spent: u64, controls: &Signal<Controls>) -> Element {
  let largest = rows.first().map_or(1, BreakdownRow::total).max(1);
  let (name, mono) = label(ctx, &row.key);
  let name = Text::new(&name)
    .variant(if mono {
      ExtraTypography::MonoValue.role()
    } else {
      TypographyStyle::Caption
    })
    .color(PaletteColor::TextPrimary)
    .nowrap()
    .flex_shrink(1.0);
  let parts = CliSeries::ALL
    .into_iter()
    .filter(|series| row.by_cli[series.index()] > 0)
    .map(|series| {
      Element::from(
        Row::new()
          .flex(row.by_cli[series.index()] as f32)
          .height(Dimension::Pct(100.0))
          .background(series_color(series)),
      )
    });
  let bar = Row::new()
    .width(Dimension::Pct(row.total() as f32 / largest as f32 * 100.0))
    .height(Dimension::Px(BREAKDOWN_BAR_HEIGHT))
    .spacing(ExtraSpacing::Hair.role())
    .rounded(ExtraRadius::Mark.role())
    .with_children(parts);
  let content = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(name)
        .child(Spacer::new().flex(1.0))
        .child(
          Text::new(&format::tokens(row.total()))
            .variant(ExtraTypography::MonoValue.role())
            .nowrap(),
        )
        .child(
          Text::new(&format::share(ctx, percent(row.total(), spent)))
            .variant(TypographyStyle::Caption)
            .nowrap(),
        ),
    )
    .child(bar);
  // Runs whose pipeline is gone cannot be filtered by it, nor the reviewer by a profile: their rows only read.
  if matches!(row.key, BreakdownKey::Pipeline(None) | BreakdownKey::Reviewer(_)) {
    return content.into();
  }
  let (controls, key) = (controls.clone(), row.key.clone());
  pressable(surface().width(Dimension::Pct(100.0)), move || {
    controls.update(|controls| add_filter(&mut controls.filter, &key));
  })
  .padding_vertical(ExtraSpacing::Hair.role())
  .rounded(lurq::app::theme::RadiusSize::Sm)
  .hovered_style(Style::new().background(PaletteColor::AccentMuted))
  .child(content)
  .into()
}

/// The row's value as a filter, unless it is chosen already.
fn add_filter(filter: &mut orchester_core::usage::UsageFilter, key: &BreakdownKey) {
  let add = |values: &mut Vec<Arc<str>>, value: &Arc<str>| {
    if !values.contains(value) {
      toggle(values, value.clone());
    }
  };
  match key {
    BreakdownKey::ProfileStep { profile, .. } => add(&mut filter.profiles, profile),
    BreakdownKey::Model(model) => add(&mut filter.models, model),
    BreakdownKey::Pipeline(Some(pipeline)) => add(&mut filter.pipelines, pipeline),
    // Not pressable (see `bar_row`): there is no pipeline or profile to choose.
    BreakdownKey::Pipeline(None) | BreakdownKey::Reviewer(_) => {}
    BreakdownKey::Project(id, _) => {
      if !filter.projects.contains(id) {
        filter.projects.push(*id);
      }
    }
  }
}

/// A column as a table with exact numbers.
fn rows_table(ctx: &Ctx, rows: &[BreakdownRow], spent: u64) -> Element {
  let columns = [
    column(ctx.t("usage.table.name"), Width::Flex(1.4), false),
    column(ctx.t("usage.measure.spent"), Width::Flex(1.0), true),
    column(ctx.t("usage.table.share"), Width::Flex(0.6), true),
  ];
  let rows = rows
    .iter()
    .map(|entry| {
      data_row(
        &columns,
        vec![
          Cell::Text(label(ctx, &entry.key).0),
          Cell::Text(format::exact(ctx, entry.total())),
          Cell::Text(format::share(ctx, percent(entry.total(), spent))),
        ],
      )
    })
    .collect();
  table("usage-breakdown-table", &columns, rows)
}
