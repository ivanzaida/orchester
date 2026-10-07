//! The Usage dashboard (`docs/design/orchester-workspace.md`, "Usage dashboard"): a project's page
//! (`C2pTYF`, sidebar item after Pull requests) and All projects › Usage (`p4eVV6`). A pinned
//! toolbar with the period, the grouping, the filters and the range read-back; then *Summary*,
//! *Tokens over time* beside *Plan limits*, *Where tokens go*, *Most expensive* and *Efficiency*,
//! which scroll beside the gutter. Tokens only, never money; every figure from stored runs.

mod breakdown;
mod card;
mod custom_range;
mod efficiency;
mod expensive;
mod expensive_columns;
mod failed;
mod filters;
mod format;
mod limits;
mod limits_chart;
mod limits_draw;
mod limits_legend;
mod line_draw;
pub mod load;
mod measure;
mod plot;
mod state;
mod summary;
mod table;
mod threshold;
mod tokens_chart;
mod tokens_draw;
mod toolbar;

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, ScrollVertical},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  time::Timestamp,
  usage::{Period, local_date},
};

use self::{
  card::{Card, card_row, empty_card, reading, state},
  load::{Scope, UsageData},
  state::{Controls, Dashboard, Views},
  tokens_chart::{TokensChart, TokensProps},
  toolbar::{ToolbarProps, UsageToolbar},
};
use crate::{
  background,
  navigation::Navigation,
  theme::gutter_scrollbar,
  ui::{
    button::ActionButton,
    gutter::{beside_gutter, clear_of_gutter},
    icon::Icon,
    message_text,
    panel::RecoveryPanel,
  },
  workspace::Workspace,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageProps {
  pub scope: Scope,
  /// Changes when the runners changed a run, so the usage is read again.
  pub revision: u64,
}

impl lurq::app::component::DevtoolsInspectable for UsageProps {}

pub struct UsageScreen {
  controls: Signal<Controls>,
  views: Signal<Views>,
  /// Counts saved thresholds, so the dashboard reads the saved one.
  saved: Signal<u64>,
}

impl Component for UsageScreen {
  type Props = UsageProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      controls: ctx.signal(Controls::default()),
      views: ctx.signal(Views::default()),
      saved: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<UsageProps>().clone();
    let controls = self.controls.get();
    let now = Timestamp::now();
    let range = controls.period.range(local_date(now));
    let workspace = ctx
      .use_context::<Navigation>()
      .expect("the root provides Navigation before mounting screens")
      .workspace()
      .clone();
    let key = (
      workspace.clone(),
      props.scope.clone(),
      range.previous().start(),
      props.revision,
      self.saved.get(),
    );
    let load = ctx.future(
      key,
      |(workspace, scope, since, ..): (Workspace, Scope, Timestamp, u64, u64)| async move {
        background::run(move || load::read(&workspace, &scope, since)).await?
      },
    );
    let state = load.state().get();
    let dashboard = state
      .data
      .as_ref()
      .map(|data: &UsageData| Arc::new(Dashboard::of(props.scope.clone(), &controls, data, range, now)));
    let toolbar = ctx.mount_keyed::<UsageToolbar>(
      "usage-toolbar",
      ToolbarProps {
        controls: self.controls.clone(),
        dashboard: dashboard.clone(),
        range,
        scope: props.scope.clone(),
      },
    );
    let content = match (dashboard, state.error) {
      (None, Some(error)) => clear_of_gutter(Element::from(RecoveryPanel::new(
        Icon::AlertCircle,
        &ctx.t("usage.loadFailed"),
        &message_text(ctx, &error),
      ))),
      (Some(dashboard), _) => self.sections(ctx, &dashboard, &workspace),
      (None, None) => self.loading(ctx, &props.scope, &workspace),
    };
    Column::new()
      .id("usage")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(toolbar)
      .child(
        ScrollVertical::new(
          Column::new()
            .width(Dimension::Pct(100.0))
            .padding_top(SpacingSize::Lg)
            .padding_horizontal(props.scope.inset())
            .padding_right(beside_gutter(ctx, props.scope.inset()))
            .padding_bottom(SpacingSize::Section)
            .child(content),
        )
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
      )
  }
}

fn page(sections: Vec<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .with_children(sections)
    .into()
}

impl UsageScreen {
  /// While reading: dashes in the summary and a loader in each card, no skeletons (`qQOKw`).
  fn loading(&self, ctx: &mut Ctx, scope: &Scope, workspace: &Workspace) -> Element {
    let tokens_text = match scope {
      Scope::Project(open) => ctx.t_args("usage.loading.project", [("project", open.project.name().as_str())]),
      Scope::All => ctx.t("usage.loading.all"),
    };
    let loading = |ctx: &Ctx, id, key: &str| Card::new(id, ctx.t(key)).fill().body([reading(&tokens_text)]);
    let tokens = loading(ctx, "usage-tokens", "usage.tokens.title");
    let limits =
      limits::header(ctx, None, &self.views, (workspace, &self.saved)).body([reading(&ctx.t("usage.loading.limits"))]);
    page(vec![
      summary::summary(ctx, None),
      card_row(ctx, vec![tokens, limits]),
      loading(ctx, "usage-breakdown", "usage.breakdown.title"),
      loading(ctx, "usage-expensive", "usage.expensive.title"),
    ])
  }

  /// The sections, or, when there is nothing to chart, why and the plan limits last reported.
  fn sections(&self, ctx: &mut Ctx, dashboard: &Arc<Dashboard>, workspace: &Workspace) -> Element {
    let summary = summary::summary(ctx, Some(dashboard));
    if dashboard.filtered_out || dashboard.no_runs() {
      let empty = if dashboard.filtered_out {
        self.unmatched(ctx, dashboard)
      } else {
        self.no_runs(ctx, dashboard)
      };
      let menu = limits::menu(ctx, dashboard, (workspace, &self.saved));
      return page(vec![summary, empty, limits_legend::compact(ctx, dashboard, menu)]);
    }
    let tokens = ctx.mount_keyed::<TokensChart>(
      "usage-tokens-chart",
      TokensProps {
        dashboard: Arc::clone(dashboard),
        views: self.views.clone(),
      },
    );
    let limits = limits::header(ctx, Some(dashboard), &self.views, (workspace, &self.saved));
    let limits = limits::limits_card(ctx, dashboard, &self.views, limits);
    page(vec![
      summary,
      card_row(ctx, vec![tokens, limits]),
      breakdown::breakdown(ctx, dashboard, &self.views, &self.controls),
      expensive::expensive(ctx, dashboard, &self.views),
      efficiency::efficiency(ctx, dashboard, &self.views),
    ])
  }

  /// *No usage matches these filters*, naming them and what the period has without them (`YxAed`).
  fn unmatched(&self, ctx: &Ctx, dashboard: &Dashboard) -> Element {
    let controls = self.controls.clone();
    let clear = ActionButton::new(ctx.t("usage.filters.clear"))
      .icon(Icon::X)
      .id("usage-filters-clear-empty")
      .on_click(move || controls.update(|controls| controls.filter = Default::default()))
      .render(ctx);
    let filters = filters::active_filters(ctx, dashboard);
    let range = format::range(ctx, dashboard.range);
    let count = dashboard.unfiltered_runs.to_string();
    let detail = match dashboard.project_name() {
      Some(project) => ctx.t_args(
        "usage.filters.nothingDetailProject",
        [
          ("filters", filters.as_str()),
          ("range", &*range),
          ("project", &*project),
          ("count", count.as_str()),
        ],
      ),
      None => ctx.t_args(
        "usage.filters.nothingDetailAll",
        [
          ("filters", filters.as_str()),
          ("range", &*range),
          ("count", count.as_str()),
        ],
      ),
    };
    empty_card(
      "usage-empty",
      state(
        Icon::SearchX,
        &ctx.t("usage.filters.noUsage"),
        Some(&detail),
        Some(clear),
      ),
    )
  }

  /// *No runs in Kontur today*, when the last run was, and *Show last 7 days* (`AY6FM`).
  fn no_runs(&self, ctx: &Ctx, dashboard: &Dashboard) -> Element {
    let title = match (dashboard.project_name(), dashboard.is_today()) {
      (Some(project), true) => ctx.t_args("usage.noRuns.projectToday", [("project", &*project)]),
      (Some(project), false) => ctx.t_args("usage.noRuns.project", [("project", &*project)]),
      (None, true) => ctx.t("usage.noRuns.allToday"),
      (None, false) => ctx.t("usage.noRuns.all"),
    };
    let last = match (dashboard.report.last_turn, dashboard.project_name()) {
      (None, _) => ctx.t("usage.noRuns.never"),
      (Some(at), Some(project)) => ctx.t_args(
        "usage.noRuns.lastProject",
        [
          ("project", &*project),
          ("time", &*format::reported(ctx, at, dashboard.now)),
        ],
      ),
      (Some(at), None) => ctx.t_args(
        "usage.noRuns.lastAll",
        [("time", &*format::reported(ctx, at, dashboard.now))],
      ),
    };
    let (controls, today) = (self.controls.clone(), local_date(dashboard.now));
    let week = (dashboard.range.days() < 7).then(|| {
      ActionButton::new(ctx.t("usage.noRuns.showWeek"))
        .id("usage-show-week")
        .on_click(move || controls.update(|controls| *controls = controls.with_period(Period::Week, today)))
        .render(ctx)
    });
    empty_card("usage-empty", state(Icon::ChartColumn, &title, Some(&last), week))
  }
}
