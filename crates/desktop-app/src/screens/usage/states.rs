//! The page's states other than its charts (`Usage · states` `cK32U`): while reading, the figures' and the cards'
//! skeletons (`super::reading`); no runs in the period, with when the project's last run reported usage and *Show
//! last 7 days*; filters that match nothing, naming them, with *Clear filters*; and the usage that could not be read,
//! as reported, with *Retry*. Without charts the plan limits shrink to what each window last reported, beside.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::Period;
use orchester_workspace::usage::{Dashboard, DashboardState};

use super::{
  Requery, Views,
  card::{Card, card_row, state, state_room},
  filters, limits,
  read::ReadFailure,
  reading, summary, tokens, words,
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  callout::{Callout, CalloutTone},
  icon::Icon,
  icon_button::IconButton,
  message::message_text,
};

/// That `dashboard` left out runs whose record is damaged, when it did: how many, and that the rest is counted.
pub fn unread_runs(ctx: &Ctx, dashboard: &Dashboard) -> Option<Element> {
  let count = dashboard.unread_runs;
  (count > 0).then(|| {
    let title = if count == 1 {
      "usage.unreadRuns.one"
    } else {
      "usage.unreadRuns.other"
    };
    Callout::new(
      "usage-unread-runs",
      CalloutTone::Warning,
      ctx.t_args(title, [("count", count.to_string())]),
    )
    .body(ctx.t("usage.unreadRunsDetail"))
    .into()
  })
}

/// The least height of the page's body when the usage could not be read (`Load failed` in `cK32U`).
const FAILED_HEIGHT: f32 = 350.0;

/// While reading (`11 Usage · loading` `P74Xv`): the summary's names over bones, and the two charts' cards with
/// their titles and switches over their bones.
pub fn loading(ctx: &mut Ctx, project: Option<&str>, views: &Signal<Views>) -> Vec<Element> {
  let reads = match project {
    Some(project) => ctx.t_args("usage.loading.tokens", [("project", project)]),
    None => ctx.t("usage.loading.tokensEverywhere"),
  };
  let tokens_bones = reading::tokens(ctx, reads.clone());
  let tokens = Card::new("usage-tokens", ctx.t("usage.tokens.title"))
    .trailing(tokens::measure_switch(ctx, views))
    .trailing(tokens::tokens_view_switch(ctx, views))
    .fill()
    .body([state_room(tokens_bones)]);
  let menu = IconButton::new("usage-limits-menu", Icon::Ellipsis, ctx.t("usage.limits.menu")).render(ctx);
  let limits_bones = reading::limits(ctx);
  let limits = Card::new("usage-limits", ctx.t("usage.limits.title"))
    .note(ctx.t("usage.limits.note"))
    .trailing(limits::limits_view_switch(ctx, views))
    .trailing(menu)
    .fill()
    .body([state_room(limits_bones)]);
  vec![
    summary::reading(ctx, &reads),
    card_row("usage-charts", vec![tokens, limits]),
  ]
}

/// The usage could not be read (`Load failed` in `cK32U`): why, as reported, and *Retry*.
pub fn load_failed(ctx: &mut Ctx, error: &ReadFailure, retry: impl Fn() + Send + Sync + 'static) -> Vec<Element> {
  let again = ActionButton::new("usage-retry", ButtonKind::Secondary, ctx.t("usage.retry"))
    .icon(Icon::RotateCcw)
    .on_click(retry);
  let body = state(
    "usage-load-failed",
    (Icon::CircleAlert, &ctx.t("usage.loadFailed"), &reported(ctx, error)),
    Some(again.into()),
  );
  vec![
    Column::new()
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(FAILED_HEIGHT))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .child(body)
      .into(),
  ]
}

/// The error as the store reported it, or in words where nothing was reported.
fn reported(ctx: &Ctx, error: &ReadFailure) -> Arc<str> {
  error
    .reported
    .clone()
    .unwrap_or_else(|| message_text(ctx, &error.message))
}

/// What stands in the charts' place when there is nothing to chart: no runs in the period, or filters that
/// match nothing; `None` when there is.
pub fn empty(ctx: &mut Ctx, dashboard: &Arc<Dashboard>, requery: &Requery) -> Option<Element> {
  let body = match dashboard.state {
    DashboardState::Charts => return None,
    DashboardState::NoRuns => no_runs(ctx, dashboard, requery),
    DashboardState::FilteredOut { runs } => unmatched(ctx, dashboard, requery, runs),
  };
  Some(Card::new("usage-tokens", ctx.t("usage.tokens.title")).body([body]))
}

/// The empty card beside the compact plan limits, each as wide as the other, at their own heights
/// (`Main` `b0DZCy`).
pub fn beside(empty: Element, limits: Element) -> Element {
  Row::new()
    .id("usage-charts")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .with_children([empty, limits].map(|card| Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(card)))
    .into()
}

/// *No runs in Kontur today*, when the last run reported usage, and *Show last 7 days* (`AU0uR`).
fn no_runs(ctx: &mut Ctx, dashboard: &Dashboard, requery: &Requery) -> Element {
  let project = dashboard.project.as_deref();
  let title = match (project, dashboard.is_today()) {
    (Some(project), true) => ctx.t_args("usage.noRuns.today", [("project", project)]),
    (Some(project), false) => ctx.t_args("usage.noRuns.period", [("project", project)]),
    (None, true) => ctx.t("usage.noRuns.todayEverywhere"),
    (None, false) => ctx.t("usage.noRuns.periodEverywhere"),
  };
  let reported = dashboard
    .report
    .last_turn
    .map(|at| words::reported(ctx, at, dashboard.now));
  let detail = match (project, reported) {
    (Some(project), Some(time)) => ctx.t_args("usage.noRuns.last", [("project", project), ("time", &*time)]),
    (Some(project), None) => ctx.t_args("usage.noRuns.never", [("project", project)]),
    (None, Some(time)) => ctx.t_args("usage.noRuns.lastEverywhere", [("time", &*time)]),
    (None, None) => ctx.t("usage.noRuns.neverEverywhere"),
  };
  let week = (dashboard.range.days() < 7).then(|| {
    let (requery, query) = (
      requery.clone(),
      dashboard.query.with_period(Period::Week, dashboard.today()),
    );
    ActionButton::new("usage-show-week", ButtonKind::Secondary, ctx.t("usage.noRuns.showWeek"))
      .icon(Icon::History)
      .on_click(move || requery.show(query.clone()))
      .into()
  });
  state("usage-no-runs", (Icon::ChartColumn, &title, &detail), week)
}

/// *No usage matches these filters*, naming them and what the period has without them (`YxAed`).
fn unmatched(ctx: &mut Ctx, dashboard: &Dashboard, requery: &Requery, runs: usize) -> Element {
  let (requery, query) = (requery.clone(), dashboard.unfiltered());
  let clear = ActionButton::new(
    "usage-clear-filters-empty",
    ButtonKind::Secondary,
    ctx.t("usage.filters.clear"),
  )
  .icon(Icon::X)
  .on_click(move || requery.show(query.clone()));
  let (range, filters, count) = (
    words::range(ctx, dashboard.range),
    filters::described(ctx, dashboard),
    runs.to_string(),
  );
  let detail = match dashboard.project.as_deref() {
    Some(project) => ctx.t_args(
      "usage.filters.nothingDetail",
      [
        ("range", &*range),
        ("filters", filters.as_str()),
        ("project", project),
        ("count", count.as_str()),
      ],
    ),
    None => ctx.t_args(
      "usage.filters.nothingDetailEverywhere",
      [
        ("range", &*range),
        ("filters", filters.as_str()),
        ("count", count.as_str()),
      ],
    ),
  };
  state(
    "usage-unmatched",
    (Icon::SearchX, &ctx.t("usage.filters.noUsage"), &detail),
    Some(clear.into()),
  )
}
