//! *Plan limits* (`C2pTYF`, `e5PbG`, `T6i75`): % used per window as lines, account-wide (limits
//! belong to the CLI account, so the project view uses every project's reports), with the windows'
//! legend above the chart, the warning threshold in the card's menu, a footnote on what is recorded
//! and where nothing is known, and the same readings as a table. Plan limits follow the CLI filter
//! only. Without any report the card says *No plan limits recorded yet*.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::usage::Grouping;

use super::{
  card::{Card, footnote, nothing, push_down, view_switch},
  format,
  limits_chart::{LimitsChart, LimitsProps, drawn, window_name},
  limits_legend::legend,
  state::{Dashboard, Views},
  table::{Cell, Width, column, data_row, table},
  threshold::{ThresholdMenu, ThresholdProps},
};
use crate::{ui::icon::Icon, workspace::Workspace};

/// The card with its title, note, switch and menu, before its body.
pub fn header(
  ctx: &mut Ctx,
  dashboard: Option<&Dashboard>,
  views: &Signal<Views>,
  (workspace, saved): (&Workspace, &Signal<u64>),
) -> Card {
  let table_shown = views.get().limits_table;
  let views = views.clone();
  let switch = view_switch(
    ctx,
    ("usage-limits-view", Icon::ChartColumn),
    table_shown,
    false,
    move |table| views.update(|views| views.limits_table = table),
  );
  let card = Card::new("usage-limits", ctx.t("usage.limits.title"))
    .note(ctx.t("usage.limits.note"))
    .trailing(switch)
    .fill();
  match dashboard {
    Some(dashboard) => card.trailing(menu(ctx, dashboard, (workspace, saved))),
    None => card,
  }
}

/// The card's … menu with the warning threshold.
pub fn menu(ctx: &mut Ctx, dashboard: &Dashboard, (workspace, saved): (&Workspace, &Signal<u64>)) -> Element {
  ctx.mount_keyed::<ThresholdMenu>(
    "usage-threshold-menu",
    ThresholdProps {
      threshold: dashboard.threshold,
      workspace: workspace.clone(),
      saved: saved.clone(),
    },
  )
}

pub fn limits_card(ctx: &mut Ctx, dashboard: &Arc<Dashboard>, views: &Signal<Views>, card: Card) -> Element {
  if dashboard.limits.is_empty() {
    return card.body([nothing(
      Icon::Gauge,
      &ctx.t("usage.limits.none"),
      Some(&ctx.t("usage.limits.noneDetail")),
      None,
    )]);
  }
  let note = footnote("usage-limits-footnote", &footnote_text(ctx, dashboard));
  if views.get().limits_table {
    return card.body([limits_table(ctx, dashboard), push_down(), note]);
  }
  let chart = ctx.mount_keyed::<LimitsChart>(
    "usage-limits-chart",
    LimitsProps {
      dashboard: Arc::clone(dashboard),
    },
  );
  card.body([legend(ctx, dashboard), chart, push_down(), note])
}

/// What a point is, what the dotted, hatched and dashed marks mean, and when 5-hour windows show.
fn footnote_text(ctx: &Ctx, dashboard: &Dashboard) -> Arc<str> {
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
  let mut sentences = vec![
    ctx.t("usage.limits.footnote.points"),
    ctx.t("usage.limits.footnote.dotted"),
  ];
  if !gaps.is_empty() {
    sentences.push(ctx.t_args(
      "usage.limits.footnote.hatched",
      [("gaps", gaps.join(&ctx.t("usage.listSeparator")).as_str())],
    ));
  }
  sentences.push(ctx.t("usage.limits.footnote.threshold"));
  sentences.push(ctx.t(match dashboard.grouping {
    Grouping::Day => "usage.limits.footnote.byDay",
    Grouping::Hour => "usage.limits.footnote.byHour",
  }));
  sentences.join(&ctx.t("usage.sentenceSeparator")).into()
}

/// The readings as a table: when, which window, % used, its reset and the run that reported it.
fn limits_table(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let columns = [
    column(ctx.t("usage.table.reported"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.window"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.used"), Width::Flex(0.5), true),
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
      data_row(
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
