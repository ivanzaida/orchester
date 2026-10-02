//! The invocation panel beside the transcript (`n0cEOG`): the step and attempt, agent, provider,
//! model and start of the latest agent launch, the code index it got, its reported tokens, the
//! run's total, the tokens before its step's first edit, and the agent's permissions. While the
//! transcript is narrowed to a step (`F3XhQ`) the panel is about that step's latest launch, named
//! after the step, with when it ran once it ended. Token counts are what the CLI reported; cached
//! input is not counted.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{id::StepExecutionId, run::Invocation, time::Timestamp, tokens::compact_tokens};

use super::{
  super::{super::ProjectNav, permissions::permissions_value},
  engine, first_edit, native,
};
use crate::{
  display,
  tasks::{detail::RunView, transcript::Entry},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{message_text, property::property},
};

/// Panel width and its dividers (`n0cEOG`, `ZR50O`).
const PANEL_WIDTH: f32 = 320.0;
const DIVIDER_HEIGHT: f32 = 1.0;

pub(super) fn invocation_panel(
  ctx: &Ctx,
  nav: &ProjectNav,
  run: &RunView,
  entries: &[Entry],
  filter: Option<StepExecutionId>,
) -> Element {
  let invocation = match filter {
    Some(execution) => run.invocations_of(execution).last(),
    None => run.invocations.last(),
  };
  let title = match (
    filter,
    invocation.and_then(|invocation| run.step_of(invocation.execution)),
  ) {
    // An agent Orchester runs itself names its step (04.H10 `FRtSj`).
    (_, Some(step)) if native::is_native(step) => native::title(ctx, step),
    (Some(_), Some(step)) => ctx.t_args(
      "runs.activity.panel.stepTitle",
      [
        ("step", &*step.name),
        ("agent", step.agent.as_deref().unwrap_or_default()),
      ],
    ),
    _ => ctx.t("runs.activity.panel.title"),
  };
  let panel = Column::new()
    .id("activity-invocation-content")
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Step.role())
    .spacing(ExtraSpacing::Group.role())
    .child(
      Text::new(&title)
        .variant(ExtraTypography::SectionHeading.role())
        .id("activity-invocation-title"),
    );
  let Some(invocation) = invocation else {
    return frame(panel.child(Text::new(&ctx.t("runs.activity.panel.none")).variant(ExtraTypography::Detail.role())));
  };
  // A tool step's command runs no model and changes no file an agent reports.
  let before = invocation
    .model()
    .and_then(|_| first_edit::before_first_edit(ctx, run, invocation.execution));
  let step = run.step_of(invocation.execution);
  let native_step = step.filter(|step| native::is_native(step));
  let note = match (&before, native_step.is_some()) {
    (None, false) => "runs.activity.panel.note",
    (Some((_, false)), false) => "runs.activity.panel.noteFirstEdit",
    (Some((_, true)), false) => "runs.activity.panel.noteAtMost",
    (None, true) => "runs.activity.panel.noteNative",
    (Some((_, false)), true) => "runs.activity.panel.noteNativeFirstEdit",
    (Some((_, true)), true) => "runs.activity.panel.noteNativeAtMost",
  };
  let before = before.map(|(rows, _)| rows);
  let permissions = match native_step {
    Some(step) => native::permissions_row(ctx, nav, step)
      .into_iter()
      .chain(native::tool_rows(ctx, step, entries, invocation.id))
      .collect(),
    None => step
      .and_then(|step| permissions_value(ctx, step))
      .map(|value| property(&ctx.t("runs.summary.permissions"), &value))
      .into_iter()
      .collect::<Vec<_>>(),
  };
  let panel = panel
    .with_children(details(ctx, run, invocation, filter.is_some()))
    .child(divider())
    .with_children(tokens(ctx, run, invocation))
    .with_children(before.is_some().then(divider))
    .with_children(before.into_iter().flatten())
    .child(divider())
    .with_children(permissions)
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&ctx.t(note))
        .variant(ExtraTypography::Hint.role())
        .id("activity-invocation-note"),
    );
  frame(panel)
}

/// The panel's fill and edge around `content`, which scrolls when it is taller than the panel, so
/// the note keeps the padding under it.
fn frame(content: Column) -> Element {
  Column::new()
    .id("activity-invocation")
    .width(Dimension::Px(PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(ScrollVertical::new(content).width(Dimension::Pct(100.0)).flex(1.0))
    .into()
}

/// The step, agent and launch; when it started, or when a step the transcript is narrowed to ran
/// once it ended; the code index it got.
fn details(ctx: &Ctx, run: &RunView, invocation: &Invocation, narrowed: bool) -> Vec<Element> {
  let Some(step) = run.step_of(invocation.execution) else {
    return Vec::new();
  };
  let attempt = run
    .run
    .attempts()
    .iter()
    .find(|attempt| attempt.execution(invocation.execution).is_some())
    .map_or_else(String::new, |attempt| attempt.number().to_string());
  let number = format!("{:02}", step.number);
  let step_value = ctx.t_args(
    "runs.activity.panel.stepValue",
    [
      ("number", number.as_str()),
      ("step", &*step.name),
      ("attempt", attempt.as_str()),
    ],
  );
  // A tool step's command runs no model: the panel names its tool instead of an agent.
  let (label, engine) = match invocation.model() {
    Some(model) if native::is_native(step) => ("runs.activity.panel.agent", model.as_str().into()),
    Some(model) => ("runs.activity.panel.agent", engine(ctx, step.provider, model.as_str())),
    None => ("runs.activity.panel.tool", ctx.t("runs.activity.commandTool")),
  };
  let agent = ctx.t_args(
    "runs.activity.panel.agentValue",
    [
      ("agent", step.agent.as_deref().unwrap_or_default()),
      ("engine", &*engine),
    ],
  );
  let invocation_number = invocation.number.to_string();
  let when = match ran(run, invocation.execution).filter(|_| narrowed) {
    Some((start, end)) => {
      let took =
        std::time::Duration::from_millis(u64::try_from(end.unix_millis() - start.unix_millis()).unwrap_or_default());
      let value = ctx.t_args(
        "runs.activity.panel.ranValue",
        [
          ("start", display::clock(start).as_str()),
          ("end", display::clock(end).as_str()),
          ("took", &*message_text(ctx, &display::duration(took))),
        ],
      );
      property(&ctx.t("runs.activity.panel.ran"), &value)
    }
    None => property(
      &ctx.t("runs.activity.panel.started"),
      &display::clock(invocation.started_at),
    ),
  };
  let mut rows = vec![
    property(&ctx.t("runs.activity.panel.step"), &step_value),
    property(&ctx.t(label), &agent),
  ];
  if native::is_native(step) {
    // 04.H10 `f0xJK`: where it ran, then when it started and for how long; no invocation number.
    rows.extend(native::launch_rows(ctx, step));
    rows.push(native::started_row(ctx, invocation));
  } else {
    rows.push(property(&ctx.t("runs.activity.panel.invocation"), &invocation_number));
    rows.push(when);
  }
  rows.extend(first_edit::code_index_row(ctx, run, invocation));
  rows
}

/// When `execution` started and ended, once all its launches ended.
fn ran(run: &RunView, execution: StepExecutionId) -> Option<(Timestamp, Timestamp)> {
  let invocations: Vec<&Invocation> = run.invocations_of(execution).collect();
  let start = invocations.first()?.started_at;
  let end = invocations
    .iter()
    .map(|invocation| invocation.ended_at)
    .collect::<Option<Vec<_>>>()?
    .into_iter()
    .max()?;
  Some((start, end))
}

fn tokens(ctx: &Ctx, run: &RunView, invocation: &Invocation) -> Vec<Element> {
  let usage = invocation.usage;
  let count = compact_tokens;
  let counted = ctx.t_args("runs.tokens", [("count", count(usage.counted()).as_str())]);
  let cached = ctx.t_args("runs.cachedValue", [("count", count(usage.cached_input).as_str())]);
  let total = ctx.t_args("runs.tokens", [("count", count(run.usage().counted()).as_str())]);
  // A command tool's program uses no tokens: only the run's total says something.
  if invocation.model().is_none() {
    return vec![property(&ctx.t("runs.activity.panel.runTotal"), &total)];
  }
  let run_row = match run.step_of(invocation.execution) {
    Some(step) if native::is_native(step) => native::budget_row(ctx, run),
    _ => property(&ctx.t("runs.activity.panel.runTotal"), &total),
  };
  vec![
    property(&ctx.t("runs.activity.panel.input"), &count(usage.counted_input())),
    property(&ctx.t("runs.activity.panel.output"), &count(usage.output)),
    property(&ctx.t("runs.cached"), &cached),
    property(&ctx.t("runs.activity.panel.counted"), &counted),
    run_row,
  ]
}

/// A 1px hairline between groups (`ZR50O`, `TTt5s`).
fn divider() -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(DIVIDER_HEIGHT))
    .background(ExtraColor::Hairline.role())
    .into()
}
