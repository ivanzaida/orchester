//! *Plan limits* (`C2pTYF`, `e5PbG`, `T6i75`): % used per window as lines, account-wide (limits
//! belong to the CLI account, so the project view uses every project's reports), with a legend of
//! each window's current value and reset, the warning threshold in the card's menu, a footnote on
//! what is recorded and where nothing is known, and the same readings as a table. Plan limits follow
//! the CLI filter only. Without any report the card says *No plan limits recorded yet*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::Text,
  core::Signal,
  node::Element,
};
use orchester_core::usage::{
  CliSeries, Grouping,
  limits::{CurrentLimit, WindowKind},
};

use super::{
  card::{Card, legend, legend_item, nothing, series_color, view_switch},
  format,
  limits_chart::{LimitsChart, LimitsProps, drawn, window_name},
  state::{Dashboard, Views},
  table::{Cell, Width, column, row, table},
  threshold::{ThresholdMenu, ThresholdProps},
};
use crate::workspace::Workspace;

pub fn limits_card(
  ctx: &mut Ctx,
  dashboard: &Arc<Dashboard>,
  views: &Signal<Views>,
  (workspace, saved): (&Workspace, &Signal<u64>),
) -> Element {
  let table_shown = views.get().limits_table;
  let views = views.clone();
  let switch = view_switch(ctx, "usage-limits-view", table_shown, move |table| {
    views.update(|views| views.limits_table = table);
  });
  let menu = ctx.mount_keyed::<ThresholdMenu>(
    "usage-threshold-menu",
    ThresholdProps {
      threshold: dashboard.threshold,
      workspace: workspace.clone(),
      saved: saved.clone(),
    },
  );
  let note = if dashboard.all_projects() {
    ctx.t("usage.limits.noteAll")
  } else {
    ctx.t("usage.limits.noteProject")
  };
  let card = Card::new("usage-limits", ctx.t("usage.limits.title"))
    .note(note)
    .trailing(switch)
    .trailing(menu);
  if dashboard.limits.is_empty() {
    return card.body([nothing(
      &ctx.t("usage.limits.none"),
      Some(&ctx.t("usage.limits.noneDetail")),
      None,
    )]);
  }
  if table_shown {
    return card.body([limits_table(ctx, dashboard), footnote(ctx, dashboard)]);
  }
  let chart = ctx.mount_keyed::<LimitsChart>(
    "usage-limits-chart",
    LimitsProps {
      dashboard: Arc::clone(dashboard),
    },
  );
  card.body([chart, current(ctx, dashboard), footnote(ctx, dashboard)])
}

/// Each window's current value and reset (*Codex · weekly 41% resets Oct 4, 09:00*); grouped by
/// day, 5-hour windows are not drawn and their entries are dimmed but keep the value.
fn current(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let mut windows: Vec<&CurrentLimit> = dashboard.limits.current.iter().collect();
  windows.sort_by_key(|window| (CliSeries::of(Some(window.provider)), std::cmp::Reverse(window.minutes)));
  let items = windows
    .into_iter()
    .map(|window| {
      let dimmed = dashboard.grouping == Grouping::Day && WindowKind::of(window.minutes) == WindowKind::FiveHour;
      let name = ctx.t_args(
        "usage.limits.legendName",
        [
          ("cli", &*format::cli_name(ctx, window.provider)),
          ("window", &*window_name(ctx, window.minutes)),
        ],
      );
      legend_item(
        series_color(CliSeries::of(Some(window.provider))),
        &name,
        &value(ctx, dashboard, window),
        dimmed,
      )
    })
    .collect();
  legend(items, None)
}

/// `41% resets Oct 4, 09:00`; once the reset has passed the share is no longer known.
fn value(ctx: &Ctx, dashboard: &Dashboard, window: &CurrentLimit) -> Arc<str> {
  match window.resets {
    Some(resets) if resets <= dashboard.now => {
      ctx.t_args("usage.limits.wasReset", [("time", &*format::date_time(ctx, resets))])
    }
    Some(resets) => ctx.t_args(
      "usage.limits.current",
      [
        (
          "percent",
          &*format::share(ctx, window.percent.map(|percent| percent.round() as u64)),
        ),
        ("time", &*format::date_time(ctx, resets)),
      ],
    ),
    None => format::share(ctx, window.percent.map(|percent| percent.round() as u64)),
  }
}

/// What is recorded, and before when nothing is known for each CLI.
fn footnote(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let start = dashboard.range.start();
  let gaps: Vec<Arc<str>> = dashboard
    .limits
    .first
    .iter()
    .filter(|(_, first)| *first > start)
    .map(|(provider, first)| {
      ctx.t_args(
        "usage.limits.noDataBefore",
        [
          ("cli", &*format::cli_name(ctx, *provider)),
          ("time", &*format::weekday_time(ctx, *first)),
        ],
      )
    })
    .collect();
  let text = if gaps.is_empty() {
    ctx.t("usage.limits.footnote")
  } else {
    ctx.t_args(
      "usage.limits.footnoteGaps",
      [("gaps", gaps.join(&ctx.t("usage.listSeparator")).as_str())],
    )
  };
  Text::new(&text)
    .id("usage-limits-footnote")
    .variant(TypographyStyle::Caption)
    .color(PaletteColor::TextMuted)
    .into()
}

/// The readings as a table: when, which window, % used, its reset and the run that reported it.
fn limits_table(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let columns = [
    column(ctx.t("usage.table.when"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.window"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.used"), Width::Flex(0.6), true),
    column(ctx.t("usage.table.resets"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.reportedBy"), Width::Flex(1.2), false),
  ];
  let mut readings: Vec<_> = drawn(dashboard)
    .into_iter()
    .flat_map(|line| line.points.iter().map(move |point| (line, point)))
    .collect();
  readings.sort_by_key(|(_, point)| std::cmp::Reverse(point.at));
  let rows = readings
    .into_iter()
    .map(|(line, point)| {
      let window = ctx.t_args(
        "usage.limits.legendName",
        [
          ("cli", &*format::cli_name(ctx, line.provider)),
          ("window", &*window_name(ctx, line.minutes)),
        ],
      );
      let source = point.source.as_ref().map_or_else(
        || ctx.t("usage.unknown"),
        |source| {
          ctx.t_args(
            "usage.limits.source",
            [
              ("project", &*source.project),
              ("task", &*source.task_key),
              ("run", source.run.get().to_string().as_str()),
            ],
          )
        },
      );
      row(
        &columns,
        vec![
          Cell::Text(format::date_time(ctx, point.at)),
          Cell::Text(window),
          Cell::Text(format::share(ctx, Some(point.percent.round() as u64))),
          Cell::Muted(
            point
              .resets
              .map_or_else(|| ctx.t("usage.unknown"), |at| format::date_time(ctx, at)),
          ),
          Cell::Muted(source),
        ],
      )
    })
    .collect();
  table("usage-limits-table", &columns, rows)
}
