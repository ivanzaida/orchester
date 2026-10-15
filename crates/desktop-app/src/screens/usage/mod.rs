//! The Usage page (ORC-15, layer `Usage` `et35n`: `Usage · project · dark` `CFRdj`, its table view
//! `d3lc3`, states `cK32U`, interactions `p732q`, every project's `All projects · Usage` `rHA8G`): the
//! tokens and the active agent time of a project's runs, or of every project's ([`place`]), over a period,
//! against as many days before. A pinned toolbar with the period, *Group by*, the
//! filters and the period read back; then *Summary*, *Tokens over time* beside *Plan limits*, *Where
//! tokens go*, *Most expensive* and *Efficiency*, which scroll. Every chart reads as a table too.
//!
//! The period, grouping and filters are the route's query (`orchester_workspace::usage::UsageQuery`), so a
//! link opens the page as it was asked for, and a change of them takes the tab's current entry's place.
//! What counts, which state the page is in and what a filter chooses is `orchester_workspace::usage`; the
//! usage is read off the UI thread, again whenever the project's runs change, and the dashboard is built
//! off it too, again whenever the query or the usage read changes.

mod breakdown;
mod card;
mod custom_range;
mod efficiency;
mod expensive;
mod failed;
mod filters;
mod limits;
mod limits_chart;
mod limits_draw;
mod limits_legend;
mod line_draw;
mod place;
mod plot;
mod read;
mod states;
mod summary;
mod table;
mod threshold;
mod tokens;
mod tokens_draw;
mod toolbar;
mod tooltip;
mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, ScrollVertical},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{time::Timestamp, usage::Measure};
use orchester_workspace::usage::{Dashboard, UsageQuery};

pub use self::place::{Place, Requery};
use self::toolbar::{ToolbarProps, UsageToolbar};
use crate::{project::ShownProject, theme::UsageSpacing};

pub fn screen(ctx: &mut Ctx, project: &ShownProject, query: &UsageQuery) -> Element {
  let key = format!("usage-{}", project.open.project.id());
  ctx.mount_keyed::<UsageScreen>(
    &key,
    UsageProps {
      place: Place::Project(project.clone()),
      query: query.clone(),
    },
  )
}

/// Every project's usage, in All projects' *Usage* tab, read again whenever a runner's work changes
/// (`work`, the runners' work revision).
pub fn everywhere(ctx: &mut Ctx, query: &UsageQuery, work: u64) -> Element {
  ctx.mount_keyed::<UsageScreen>(
    "usage-everywhere",
    UsageProps {
      place: Place::Everywhere(work),
      query: query.clone(),
    },
  )
}

#[derive(Clone, PartialEq)]
pub struct UsageProps {
  place: Place,
  query: UsageQuery,
}

impl DevtoolsInspectable for UsageProps {}

/// How the sections show their data, which the page keeps while the query changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Views {
  pub measure: Measure,
  pub tokens_table: bool,
  pub limits_table: bool,
  pub breakdown_table: bool,
  pub cache_table: bool,
  pub edits_table: bool,
  /// *Most expensive* lists runs instead of tasks.
  pub runs: bool,
  /// *Most expensive* lists every row instead of the first five.
  pub all_rows: bool,
}

impl DevtoolsInspectable for Views {}

pub struct UsageScreen {
  views: Signal<Views>,
  /// Counts saved warning thresholds, so the page reads the saved one.
  saved: Signal<u64>,
  retry: Signal<u64>,
}

impl Component for UsageScreen {
  type Props = UsageProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      views: ctx.signal(Views::default()),
      saved: ctx.signal(0),
      retry: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let UsageProps { place, query } = ctx.props::<UsageProps>().clone();
    // Again whenever the runs change, a threshold is saved or Retry is pressed.
    let deps = (place.changes(), self.saved.get(), self.retry.get());
    let since = Dashboard::since(&query, Timestamp::now());
    let loaded = read::read(ctx, place.root(), since, deps);
    let name = place.name();
    let built = read::build(ctx, (name.clone(), query.clone()), loaded.data.clone());
    let dashboard = built.data.clone().flatten().map(|built| built.0);
    let requery = place.requery(ctx);
    let toolbar = ctx.mount_keyed::<UsageToolbar>(
      "usage-toolbar",
      ToolbarProps {
        project: name.clone(),
        query,
        dashboard: dashboard.clone(),
        requery: requery.clone(),
      },
    );
    let failed = read::failure(&loaded, &built, dashboard.is_some());
    let content = match (dashboard, failed) {
      (Some(dashboard), _) => self.sections(ctx, &dashboard, &requery),
      (None, Some(error)) => {
        let retry = self.retry.clone();
        states::load_failed(ctx, &error, move || retry.update(|count| *count += 1))
      }
      (None, None) => states::loading(ctx, name.as_deref(), &self.views),
    };
    Column::new()
      .id(if name.is_some() {
        "screen-usage"
      } else {
        "usage-everywhere"
      })
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(toolbar)
      .child(
        ScrollVertical::new(
          Column::new()
            .id("usage-sections")
            .width(Dimension::Pct(100.0))
            .padding(
              Padding::new()
                .top(SpacingSize::Xl)
                .bottom(SpacingSize::Section)
                .left(UsageSpacing::Page.role())
                .right(UsageSpacing::Page.role()),
            )
            .spacing(SpacingSize::Xl)
            .with_children(content),
        )
        .id("usage-scroll")
        .width(Dimension::Pct(100.0))
        .flex(1.0),
      )
  }
}

impl UsageScreen {
  /// The sections, or, when there is nothing to chart, why and the plan limits last reported.
  fn sections(&self, ctx: &mut Ctx, dashboard: &Arc<Dashboard>, requery: &Requery) -> Vec<Element> {
    let saved = self.saved.clone();
    let summary = summary::summary(ctx, Some(dashboard));
    if let Some(empty) = states::empty(ctx, dashboard, requery) {
      let menu = threshold::menu(ctx, dashboard, &saved);
      let compact = limits_legend::compact(ctx, dashboard, menu);
      return vec![summary, states::beside(empty, compact)];
    }
    let tokens = tokens::card(ctx, dashboard, &self.views);
    let limits = limits::card(ctx, dashboard, &self.views, &saved);
    vec![
      summary,
      card::card_row("usage-charts", vec![tokens, limits]),
      breakdown::card(ctx, dashboard, &self.views, requery),
      expensive::card(ctx, dashboard, &self.views),
      efficiency::section(ctx, dashboard, &self.views),
    ]
  }
}
