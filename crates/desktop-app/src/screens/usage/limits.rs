//! *Plan limits* (`Card · Plan limits` `CPuK9`): % used per window as lines, account-wide (limits belong to
//! the CLI account, so the page draws every project's reports), with the windows' legend over the chart,
//! the warning threshold dashed across, its menu opening Settings › General where it is set, a footnote on what is
//! recorded and where nothing is known, and
//! the newest readings as a table (`Table` `CFzvr`). Plan limits follow the CLI filter only. Without any report
//! the card says *No plan limits recorded yet* (`No plan limits reported yet` in `cK32U`).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::{connection::plan_limits::ReadingSource, usage::Grouping};
use orchester_workspace::usage::Dashboard;

use super::{
  Views,
  card::{Card, footnote, push_down, state, state_room, view_switch},
  limits_chart::{LimitsChart, LimitsProps, line_name, window_owner},
  limits_legend::legend,
  table::{Cell, Density, Width, column, row, table},
  threshold, tokens, words,
};
use crate::ui::icon::Icon;

/// Table columns (`CFzvr`): when, which window, % used, its reset.
/// REPORTED takes *yesterday 19:17*, wider than the design's *today 14:15*; WINDOW keeps the design's 140, so
/// *Codex · Default · weekly* is whole, and RESETS takes a two-digit day (*Oct 20, 09:00*); REPORTED BY gives
/// way instead.
pub const REPORTED_COLUMN: f32 = 96.0;
pub const WINDOW_COLUMN: f32 = 140.0;
const USED_COLUMN: f32 = 36.0;
pub const RESETS_COLUMN: f32 = 84.0;

pub fn limits_view_switch(ctx: &Ctx, views: &Signal<Views>) -> Element {
  let views = views.clone();
  view_switch(
    ctx,
    "usage-limits-view",
    views.get().limits_table,
    false,
    move |table| {
      views.update(|views| views.limits_table = table);
    },
  )
}

pub fn card(ctx: &mut Ctx, dashboard: &Arc<Dashboard>, views: &Signal<Views>) -> Element {
  let menu = threshold::menu(ctx, dashboard);
  let card = Card::new("usage-limits", ctx.t("usage.limits.title"))
    .note(ctx.t("usage.limits.note"))
    .trailing(limits_view_switch(ctx, views))
    .trailing(menu)
    .fill();
  if dashboard.limits.is_empty() {
    // Reports the filters leave out are not reports that never came.
    let (title, detail) = if dashboard.limits_filtered_out() {
      ("usage.limits.noneChosen", "usage.limits.noneChosenDetail")
    } else {
      ("usage.limits.none", "usage.limits.noneDetail")
    };
    return card.body([state_room(state(
      "usage-limits-none",
      (Icon::Gauge, &ctx.t(title), &ctx.t(detail)),
      None,
    ))]);
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
  card.body([tokens::chart_block(legend(ctx, dashboard), chart), note])
}

/// What a point is, what the dotted, hatched and dashed marks mean, and when 5-hour windows show.
fn footnote_text(ctx: &Ctx, dashboard: &Dashboard) -> Arc<str> {
  let start = dashboard.range.start();
  let late: Vec<_> = dashboard
    .limits
    .first_by_account
    .iter()
    .filter(|(_, _, first)| *first > start)
    .collect();
  // One stretch reads as the hatch's label names it; several each name their account.
  let gaps: Vec<Arc<str>> = late
    .iter()
    .map(|(provider, account, first)| {
      let time = words::weekday_time(ctx, *first);
      if late.len() == 1 {
        ctx.t_args("usage.limits.noDataBefore", [("time", &*time)])
      } else {
        let owner = window_owner(ctx, dashboard, (*provider, *account));
        ctx.t_args(
          "usage.limits.noDataFromBefore",
          [("account", &*owner), ("time", &*time)],
        )
      }
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

/// The readings `Dashboard::limit_table` picks, as a table: when, which window, % used, its reset and the run
/// that reported it.
fn limits_table(ctx: &Ctx, dashboard: &Dashboard) -> Element {
  let columns = [
    column(ctx.t("usage.table.reported"), Width::Px(REPORTED_COLUMN), false),
    column(ctx.t("usage.table.window"), Width::Px(WINDOW_COLUMN), false),
    column(ctx.t("usage.table.used"), Width::Px(USED_COLUMN), true),
    column(ctx.t("usage.table.resets"), Width::Px(RESETS_COLUMN), false),
    column(ctx.t("usage.table.reportedBy"), Width::Flex(1.0), false),
  ];
  let rows = dashboard
    .limit_table()
    .into_iter()
    .enumerate()
    .map(|(index, (line, point))| {
      let source = match (&point.source, point.origin) {
        (_, ReadingSource::Read) => ctx.t("usage.limits.sourceRead"),
        (Some(source), ReadingSource::Turn) => ctx.t_args(
          "usage.limits.source",
          [
            ("project", &*source.project),
            ("run", &*words::run_label(ctx, &source.task_key, source.run.get())),
          ],
        ),
        (None, ReadingSource::Turn) => ctx.t("usage.unknown"),
      };
      let cells = vec![
        Cell::Muted(words::reported(ctx, point.at, dashboard.now)),
        Cell::Text(line_name(ctx, dashboard, (line.provider, line.account, line.minutes))),
        Cell::Text(words::share(ctx, Some(point.percent.round() as u64))),
        Cell::Muted(
          point
            .resets
            .map_or_else(|| ctx.t("usage.unknown"), |at| words::date_time(ctx, at)),
        ),
        Cell::Muted(source),
      ];
      row(
        &format!("usage-limits-table-row-{index}"),
        &columns,
        Density::Readings,
        cells,
      )
      .into()
    })
    .collect();
  table(ctx, "usage-limits-table", &columns, Density::Readings, rows)
}
