//! The end of a task's latest run in the Orchestrator's conversation (`Orchestrator · run ends · states` `J9n4q4`,
//! `docs/design/orchestrator-toolset.md`, "Decisions" 5): in the action card's frame (`Action card` `nQIhx`), as an
//! agent's wait ([`super::wait`]), its head with how the run ended or stopped, the task and the run (and how many
//! more runs a shared limit stopped at once), and at its end the end's icon in its color and when it was noticed;
//! what happens with it (the Orchestrator takes it up by itself while *Let the Orchestrator answer agents* is on, and
//! merges a run that succeeded on its own while *merge finished runs on its own* is on too, else it waits for the
//! person); and *Open* the task's Summary.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{AgentWait, Continuing, MergeLeft, Merging, RunEnd},
  project::TaskKey,
  run::RunLimit,
  time::Timestamp,
};
use orchester_workspace::display;

use super::card::{self, State};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// The card of `wait`, the end `end` of its run, kept at `at`, which started an Orchestrator turn by itself when
/// `turned`; in project `project`; `id` names it.
pub(super) fn card(
  ctx: &mut Ctx,
  id: &str,
  (wait, end, at, turned): (&AgentWait, RunEnd, Option<Timestamp>, bool),
  project: ProjectId,
) -> Element {
  let run = wait.number.to_string();
  let step = step_label(ctx, wait);
  let (glyph, color) = look(end);
  let title = ctx.t_args(
    title(end, (wait.step.is_some(), wait.others > 0)),
    [
      ("task", wait.task.as_str()),
      ("run", run.as_str()),
      ("step", &*step),
      ("count", &wait.others.to_string()),
    ],
  );
  let time = at.map(display::clock).unwrap_or_default();
  let state = State {
    glyph,
    color,
    words: ctx.t_args("orchestrator.end.noticed", [("time", time.as_str())]),
  };
  let effect = effect(ctx, wait, end, turned);
  card::frame(id, name(end))
    .describe("automatic", wait.automatic.to_string())
    .child(card::head(id, (glyph, &title), None, state))
    .child(
      Text::new(&effect)
        .id(format!("{id}-effect"))
        .variant(TypographyStyle::Body)
        .width(Dimension::Pct(100.0)),
    )
    .with_children(open_summary(ctx, id, project, &wait.task))
    .into()
}

/// The icon of `end` and the color of its state.
const fn look(end: RunEnd) -> (Icon, PaletteColor) {
  match end {
    RunEnd::Succeeded => (Icon::GitMerge, PaletteColor::Success),
    RunEnd::Failed => (Icon::CircleX, PaletteColor::Danger),
    RunEnd::Stopped { .. } => (Icon::Hand, PaletteColor::Warning),
    RunEnd::Limit { .. } => (Icon::Gauge, PaletteColor::Warning),
    RunEnd::Interrupted => (Icon::CirclePause, PaletteColor::Warning),
  }
}

/// The step `wait`'s run stopped at, with its place in the pipeline when it is known (`03 Implement`).
fn step_label(ctx: &Ctx, wait: &AgentWait) -> Arc<str> {
  match (wait.place, wait.step.as_deref()) {
    (Some(place), Some(step)) => ctx.t_args(
      "orchestrator.change.facts.stepLabel",
      [("number", format!("{place:02}")), ("step", step.to_owned())],
    ),
    (None, Some(step)) => Arc::from(step),
    (_, None) => Arc::from(""),
  }
}

/// The title of `end`, naming the step it stopped at when it is `named`, and the other runs the same limit stopped
/// when there are `more`.
const fn title(end: RunEnd, (named, more): (bool, bool)) -> &'static str {
  match (end, named) {
    (RunEnd::Succeeded, _) => "orchestrator.end.succeeded",
    (RunEnd::Failed, true) => "orchestrator.end.failedAt",
    (RunEnd::Failed, false) => "orchestrator.end.failed",
    (RunEnd::Stopped { .. }, true) => "orchestrator.end.stoppedAt",
    (RunEnd::Stopped { .. }, false) => "orchestrator.end.stopped",
    (RunEnd::Limit { limit }, _) => match (limit, more) {
      (RunLimit::DailyBudget, true) => "orchestrator.end.limitDailyMore",
      (RunLimit::GroupBudget, true) => "orchestrator.end.limitGroupMore",
      (RunLimit::Tokens, _) => "orchestrator.end.limitTokens",
      (RunLimit::Time, _) => "orchestrator.end.limitTime",
      (RunLimit::DailyBudget, false) => "orchestrator.end.limitDaily",
      (RunLimit::GroupBudget, false) => "orchestrator.end.limitGroup",
      (RunLimit::StepBudget { .. }, _) => "orchestrator.end.limitStep",
      (RunLimit::PlanLimit { .. }, _) => "orchestrator.end.limitPlan",
    },
    (RunEnd::Interrupted, _) => "orchestrator.end.interrupted",
  }
}

/// What happens with `end` of `wait`'s run: the Orchestrator takes it up by itself (in the turn it `turned`, or after
/// the turn under way; an interruption it continues once, or leaves to the person when it came again soon or the
/// database is under contention; a success it merges on its own, or leaves to the person and says why), the
/// Orchestrator's own decision on the run led to it so it waits for the person, or the person's switch is off.
fn effect(ctx: &Ctx, wait: &AgentWait, end: RunEnd, turned: bool) -> Arc<str> {
  let key = match (end, wait.automatic, turned, wait.continuing, &wait.merging) {
    (RunEnd::Succeeded, true, true, _, Some(Merging::Merges)) => "orchestrator.end.effectMerging",
    (RunEnd::Succeeded, true, true, _, Some(Merging::Left { why })) => {
      let why = left_words(ctx, why);
      return ctx.t_args("orchestrator.end.effectMergeLeft", [("why", &*why)]);
    }
    (RunEnd::Succeeded, true, true, ..) => "orchestrator.end.effectReadyAnswering",
    (RunEnd::Interrupted, true, true, Some(Continuing::Continues), _) => "orchestrator.end.effectContinuing",
    (RunEnd::Interrupted, true, true, Some(Continuing::InterruptedAgain), _) => {
      "orchestrator.end.effectInterruptedAgain"
    }
    (RunEnd::Interrupted, true, true, Some(Continuing::StorageContention), _) => "orchestrator.end.effectContention",
    (_, true, true, ..) => "orchestrator.end.effectAnswering",
    (_, true, false, ..) => "orchestrator.end.effectQueued",
    (_, false, ..) if wait.own => "orchestrator.end.effectOwn",
    (_, false, ..) => "orchestrator.end.effectWaits",
  };
  ctx.t(key)
}

/// Why a run that succeeded is left to the person, as its card says it.
fn left_words(ctx: &Ctx, why: &MergeLeft) -> Arc<str> {
  match why {
    MergeLeft::NotReady => ctx.t("orchestrator.end.mergeLeft.notReady"),
    MergeLeft::StepNotPassed { step } => ctx.t_args("orchestrator.end.mergeLeft.stepNotPassed", [("step", step)]),
    MergeLeft::AcceptedByAutomation { step } => {
      ctx.t_args("orchestrator.end.mergeLeft.acceptedByAutomation", [("step", step)])
    }
    MergeLeft::MarkedBlocked { reason } => ctx.t_args("orchestrator.end.mergeLeft.markedBlocked", [("reason", reason)]),
    MergeLeft::BlockedByTasks { tasks } => ctx.t_args(
      "orchestrator.end.mergeLeft.blockedByTasks",
      [("tasks", tasks.join(", "))],
    ),
    MergeLeft::Held => ctx.t("orchestrator.end.mergeLeft.held"),
    MergeLeft::OptedOut => ctx.t("orchestrator.end.mergeLeft.optedOut"),
  }
}

/// *Open* the task's Summary, where the person decides what comes next.
fn open_summary(ctx: &mut Ctx, id: &str, project: ProjectId, task: &str) -> Option<Element> {
  let key: TaskKey = task.parse().ok()?;
  let navigation = Navigation::of(ctx);
  let label = ctx.t_args("orchestrator.end.open", [("task", task)]);
  let route = Route::Project(project, ProjectPage::Task(key, TaskView::of(TaskTab::Summary)));
  Some(
    Row::new()
      .child(
        ActionButton::new(format!("{id}-open"), ButtonKind::Ghost, label)
          .icon(Icon::ArrowUpRight)
          .on_click(move || navigation.go(route.clone())),
      )
      .into(),
  )
}

/// The name tools read of `end`.
const fn name(end: RunEnd) -> &'static str {
  match end {
    RunEnd::Succeeded => "run_succeeded",
    RunEnd::Failed => "run_failed",
    RunEnd::Stopped { .. } => "run_stopped",
    RunEnd::Limit { .. } => "run_limit",
    RunEnd::Interrupted => "run_interrupted",
  }
}
