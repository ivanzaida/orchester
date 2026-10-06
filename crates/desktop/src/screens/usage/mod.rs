//! The Usage dashboard (`docs/design/orchester-workspace.md`, "Usage dashboard"): a project's page
//! (`C2pTYF`, sidebar item after Pull requests) and All projects › Usage (`p4eVV6`). A pinned
//! toolbar with the period, the grouping, the filters and the range read-back; then *Summary*,
//! *Tokens over time*, *Plan limits*, *Where tokens go*, *Most expensive* and *Efficiency*, which
//! scroll beside the gutter. Tokens only, never money; every figure from stored runs.

mod breakdown;
mod card;
mod custom_range;
mod efficiency;
mod expensive;
mod filters;
mod format;
mod limits;
mod limits_chart;
mod limits_draw;
mod line_draw;
pub mod load;
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
  card::{Card, nothing, reading},
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
      (dashboard, _) => self.sections(ctx, &props.scope, dashboard, &workspace),
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
            .spacing(SpacingSize::Md)
            .child(content),
        )
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
      )
  }
}

impl UsageScreen {
  /// The six sections; while reading each says so (`qQOKw`).
  fn sections(
    &self,
    ctx: &mut Ctx,
    scope: &Scope,
    dashboard: Option<Arc<Dashboard>>,
    workspace: &Workspace,
  ) -> Element {
    let scope_name = match scope {
      Scope::Project(open) => Some(open.project.name().as_str().to_owned()),
      Scope::All => None,
    };
    let Some(dashboard) = dashboard else {
      let loading = |id, key: &str| Card::new(id, ctx.t(key)).body([reading(ctx, scope_name.as_deref())]);
      return Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(summary::summary(ctx, None))
        .child(loading("usage-tokens", "usage.tokens.title"))
        .child(loading("usage-limits", "usage.limits.title"))
        .child(loading("usage-breakdown", "usage.breakdown.title"))
        .child(loading("usage-expensive", "usage.expensive.title"))
        .child(loading("usage-efficiency", "usage.efficiency.title"))
        .into();
    };
    let tokens = self.tokens(ctx, &dashboard);
    let limits = limits::limits_card(ctx, &dashboard, &self.views, (workspace, &self.saved));
    let (breakdown, expensive, efficiency) = if dashboard.no_runs() || dashboard.filtered_out {
      (None, None, None)
    } else {
      (
        Some(breakdown::breakdown(ctx, &dashboard, &self.views, &self.controls)),
        Some(expensive::expensive(ctx, &dashboard, &self.views)),
        Some(efficiency::efficiency(ctx, &dashboard, &self.views)),
      )
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(summary::summary(ctx, Some(&dashboard)))
      .child(tokens)
      .child(limits)
      .with_children(breakdown)
      .with_children(expensive)
      .with_children(efficiency)
      .into()
  }

  /// *Tokens over time*, or why it has nothing to draw: no runs in the period (`AY6FM`), or filters
  /// that match nothing (`YxAed`).
  fn tokens(&self, ctx: &mut Ctx, dashboard: &Arc<Dashboard>) -> Element {
    if dashboard.filtered_out {
      let controls = self.controls.clone();
      let clear = ActionButton::new(ctx.t("usage.filters.clear"))
        .icon(Icon::X)
        .id("usage-filters-clear-empty")
        .on_click(move || controls.update(|controls| controls.filter = Default::default()))
        .render(ctx);
      let detail = filters::active_filters(ctx, dashboard);
      return Card::new("usage-tokens", ctx.t("usage.tokens.title")).body([nothing(
        &ctx.t("usage.filters.nothing"),
        Some(&detail),
        Some(clear),
      )]);
    }
    if dashboard.no_runs() {
      return Card::new("usage-tokens", ctx.t("usage.tokens.title")).body([self.no_runs(ctx, dashboard)]);
    }
    ctx.mount_keyed::<TokensChart>(
      "usage-tokens-chart",
      TokensProps {
        dashboard: Arc::clone(dashboard),
        views: self.views.clone(),
      },
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
    let last = dashboard.report.last_turn.map_or_else(
      || ctx.t("usage.noRuns.never"),
      |at| ctx.t_args("usage.noRuns.last", [("time", &*format::date_time(ctx, at))]),
    );
    let (controls, today) = (self.controls.clone(), local_date(dashboard.now));
    let week = (dashboard.range.days() < 7).then(|| {
      ActionButton::new(ctx.t("usage.noRuns.showWeek"))
        .id("usage-show-week")
        .on_click(move || controls.update(|controls| *controls = controls.with_period(Period::Week, today)))
        .render(ctx)
    });
    nothing(&title, Some(&last), week)
  }
}
