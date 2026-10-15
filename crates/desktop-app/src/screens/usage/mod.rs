//! The project's Usage page (ORC-15, layer `Usage` `et35n`: `Usage · project · dark` `CFRdj`, its table view
//! `d3lc3`, states `cK32U`, interactions `p732q`): the tokens and the active agent time of the project's
//! runs over a period, against as many days before. A pinned toolbar with the period, *Group by*, the
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

use self::toolbar::{ToolbarProps, UsageToolbar};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  theme::UsageSpacing,
};

pub fn screen(ctx: &mut Ctx, project: &ShownProject, query: &UsageQuery) -> Element {
  let key = format!("usage-{}", project.open.project.id());
  ctx.mount_keyed::<UsageScreen>(
    &key,
    UsageProps {
      project: project.clone(),
      query: query.clone(),
    },
  )
}

#[derive(Clone, PartialEq)]
pub struct UsageProps {
  project: ShownProject,
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

/// Goes to the page showing another query, in the tab's current entry.
#[derive(Clone)]
pub struct Requery {
  navigation: Navigation,
  project: orchester_core::id::ProjectId,
}

impl Requery {
  /// The project whose page this is.
  pub const fn project(&self) -> orchester_core::id::ProjectId {
    self.project
  }

  pub fn show(&self, query: UsageQuery) {
    self
      .navigation
      .replace(Route::Project(self.project, ProjectPage::Usage(Box::new(query))));
  }
}

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
    let UsageProps { project, query } = ctx.props::<UsageProps>().clone();
    // Again whenever the project's runs change, a threshold is saved or Retry is pressed.
    let deps = (
      project.feed.changes.get(),
      project.feed.revision.get(),
      self.saved.get(),
      self.retry.get(),
    );
    let since = Dashboard::since(&query, Timestamp::now());
    let loaded = read::read(ctx, Some(project.open.repo_root.clone()), since, deps);
    let name: Arc<str> = project.open.project.name().as_str().into();
    let built = read::build(ctx, (name.clone(), query.clone()), loaded.data.clone());
    let dashboard = built.data.clone().flatten().map(|built| built.0);
    let requery = Requery {
      navigation: Navigation::of(ctx),
      project: project.open.project.id(),
    };
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
      (None, None) => states::loading(ctx, &name, &self.views),
    };
    Column::new()
      .id("screen-usage")
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
      efficiency::section(ctx, dashboard, &self.views, requery),
    ]
  }
}
