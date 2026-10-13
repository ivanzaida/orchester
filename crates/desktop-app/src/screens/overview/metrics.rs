//! The Overview's figures (`Metrics` `anUOY`, `Metric card` `Qhj0H`): *Running now* with the agents at
//! work, or the runs interrupted in `warning`; *Need your input*, in `warning` while anything waits;
//! *Completed today*; *Tokens today* against the project's daily budget. While the tasks or the usage are
//! read, a figure is `—` and says so (`CHfHU`); while no runner reports, *Running now* is `—` (`KGpMB`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  node::dimension::Dimension,
};
use orchester_core::time::Timestamp;
use orchester_workspace::tasks::{
  board::{self, Figures},
  data::ProjectTasks,
  overview::figure,
};

use crate::{
  project::ShownProject,
  theme::{ExtraTypography, OverviewText},
  ui::{
    card::{CardSurface, card_frame},
    message::message_text,
  },
};

/// One figure: its label, its value (`None`: not known yet), what it means and whether it calls for the
/// person.
struct Metric<'a> {
  id: &'a str,
  label: &'a str,
  value: Option<String>,
  detail: &'a str,
  warning: bool,
}

/// The four figures; `tasks` is `None` while they are read.
pub(super) fn metrics(ctx: &mut Ctx, project: &ShownProject, tasks: Option<&ProjectTasks>) -> Row {
  let usage = project.feed.usage.get();
  let (tokens, tokens_detail) = board::tokens_today(usage.as_ref(), project.open.project.name().as_str());
  let tokens_detail = message_text(ctx, &tokens_detail);
  let reading = ctx.t("overview.metrics.reading");
  let (running, needs, completed) = match tasks {
    Some(tasks) => counted(ctx, project, tasks),
    None => {
      let unknown = || (None, reading.to_string(), false);
      (unknown(), unknown(), unknown())
    }
  };
  let [running_label, needs_label, completed_label, tokens_label] = [
    "overview.metrics.running",
    "overview.metrics.needsInput",
    "overview.metrics.completed",
    "overview.metrics.tokens",
  ]
  .map(|key| ctx.t(key));
  let cards = [
    Metric {
      id: "overview-metric-running",
      label: &running_label,
      value: running.0,
      detail: &running.1,
      warning: running.2,
    },
    Metric {
      id: "overview-metric-needs",
      label: &needs_label,
      value: needs.0,
      detail: &needs.1,
      warning: needs.2,
    },
    Metric {
      id: "overview-metric-completed",
      label: &completed_label,
      value: completed.0,
      detail: &completed.1,
      warning: completed.2,
    },
    Metric {
      id: "overview-metric-tokens",
      label: &tokens_label,
      value: tokens,
      detail: &tokens_detail,
      warning: false,
    },
  ];
  let unknown = ctx.t("overview.unknown");
  Row::new()
    .id("overview-metrics")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .with_children(cards.iter().map(|metric| card(metric, &unknown)))
}

type Counted = (Option<String>, String, bool);

/// *Running now*, *Need your input* and *Completed today* of the read `tasks`.
fn counted(ctx: &Ctx, project: &ShownProject, tasks: &ProjectTasks) -> (Counted, Counted, Counted) {
  let figures = Figures::of(tasks, Timestamp::now().local_day_start());
  let status = project.feed.status.get();
  let online = project.feed.runner_online(&project.open.repo_root);
  let agents = status
    .as_ref()
    .and_then(|status| status.work())
    .map(|work| work.agents());
  let running = figures.running_now(online, agents);
  (
    (
      running.count.map(figure),
      message_text(ctx, &running.detail).to_string(),
      running.interrupted,
    ),
    (
      Some(figure(figures.needs_you)),
      ctx.t("overview.metrics.needsInputDetail").to_string(),
      figures.needs_you > 0,
    ),
    (
      Some(figure(figures.completed_today)),
      ctx.t("overview.metrics.completedDetail").to_string(),
      false,
    ),
  )
}

fn card(metric: &Metric<'_>, unknown: &str) -> Column {
  let value = Text::new(metric.value.as_deref().unwrap_or(unknown))
    .id(format!("{}-value", metric.id))
    .variant(ExtraTypography::Display.role());
  let value = if metric.warning {
    value.color(PaletteColor::Warning)
  } else {
    value
  };
  card_frame(metric.id, CardSurface::Raised)
    .width(Dimension::Px(0.0))
    .flex(1.0)
    .padding(SpacingSize::Lg)
    .spacing(SpacingSize::Xs)
    .describe("warning", if metric.warning { "true" } else { "false" })
    .child(Text::new(metric.label).variant(OverviewText::FigureLabel.role()))
    .child(value)
    .child(
      Text::new(metric.detail)
        .id(format!("{}-detail", metric.id))
        .variant(OverviewText::FigureDetail.role()),
    )
}
