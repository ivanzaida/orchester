//! The end of a task's latest run in the Orchestrator's conversation, as an event line (`RUN ENDS` in `1 · Event
//! lines` `VGbU7`; `docs/design/orchestrator-toolset.md`, "Decisions" 5): its icon in its tone, how the run ended or
//! stopped (and how many more runs a shared limit stopped at once), the task and the run, when it was noticed, and the
//! one link that acts on it, to the task's Summary: *Review ↗* a run ready to merge, *Raise ↗* a limit, *Continue ↗*
//! an interrupted run, else *Summary ↗*. What happens with it (the Orchestrator takes it up by itself while *Let the
//! Orchestrator answer agents* is on, and merges a run that succeeded on its own while *merge finished runs on its
//! own* is on too, else it waits for the person) is said on hover and once pressed. An end dealt with since (the run
//! merged, went on, or ended otherwise) no longer waits: its line turns neutral, says what became of the run instead,
//! and links to the Summary.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  id::ProjectId,
  orchestrator::{AgentWait, Continuing, MergeLeft, Merging, RunEnd},
  project::TaskKey,
  run::RunLimit,
  time::Timestamp,
};
use orchester_workspace::{display, orchestrator::chat::EndMoved};

use super::{
  line::{Line, LineLink, Tone},
  wait::run_target,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::{icon::Icon, pressable::Press},
};

/// The line of `wait`, the end `end` of its run, kept at `at`, which started an Orchestrator turn by itself when
/// `turned`, and what became of the run since when that end no longer stands (`moved`); in project `project`; `id`
/// names it.
pub(super) fn line(
  ctx: &mut Ctx,
  id: &str,
  (wait, end, at, turned): (&AgentWait, RunEnd, Option<Timestamp>, bool),
  (moved, project): (Option<EndMoved>, ProjectId),
) -> Element {
  let (glyph, tone) = look(end);
  let step = step_label(ctx, wait);
  let what = ctx.t_args(
    what(end, (wait.step.is_some(), wait.others > 0)),
    [("step", &*step), ("count", &wait.others.to_string())],
  );
  let line = Line::new(id, (glyph, tone), what)
    .outcome(name(end))
    .target(Some(run_target(ctx, wait)))
    .time(at.map(display::clock));
  match moved {
    None => line
      .link(open_summary(ctx, end, project, &wait.task))
      .detail(Some(effect(ctx, wait, end, turned))),
    Some(moved) => Line {
      tone: Tone::Plain,
      ..line
    }
    .status("moved")
    .link(summary(ctx, "orchestrator.line.summary", project, &wait.task))
    .detail(Some(ctx.t(moved_words(moved)))),
  }
  .render(ctx)
}

/// What the line of an end that no longer stands says became of its run.
const fn moved_words(moved: EndMoved) -> &'static str {
  match moved {
    EndMoved::Merged => "orchestrator.end.moved.merged",
    EndMoved::Working => "orchestrator.end.moved.working",
    EndMoved::Stopped => "orchestrator.end.moved.stopped",
    EndMoved::Paused => "orchestrator.end.moved.paused",
    EndMoved::Interrupted => "orchestrator.end.moved.interrupted",
    EndMoved::Succeeded => "orchestrator.end.moved.succeeded",
    EndMoved::Failed => "orchestrator.end.moved.failed",
    EndMoved::Cancelled => "orchestrator.end.moved.cancelled",
    EndMoved::Gone => "orchestrator.end.moved.gone",
  }
}

/// The icon of `end` and its tone.
pub(super) const fn look(end: RunEnd) -> (Icon, Tone) {
  match end {
    RunEnd::Succeeded => (Icon::CircleCheck, Tone::Success),
    RunEnd::Failed => (Icon::CircleX, Tone::Danger),
    RunEnd::Stopped { .. } => (Icon::Hand, Tone::Warning),
    RunEnd::Limit { .. } => (Icon::Gauge, Tone::Warning),
    RunEnd::Interrupted => (Icon::CirclePause, Tone::Warning),
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

/// What the line says of `end`, naming the step it stopped at when it is `named`, and the other runs the same limit
/// stopped when there are `more`.
const fn what(end: RunEnd, (named, more): (bool, bool)) -> &'static str {
  match (end, named) {
    (RunEnd::Succeeded, _) => "orchestrator.end.line.succeeded",
    (RunEnd::Failed, true) => "orchestrator.end.line.failedAt",
    (RunEnd::Failed, false) => "orchestrator.end.line.failed",
    (RunEnd::Stopped { .. }, true) => "orchestrator.end.line.stoppedAt",
    (RunEnd::Stopped { .. }, false) => "orchestrator.end.line.stopped",
    (RunEnd::Limit { limit }, _) => match (limit, more) {
      (RunLimit::DailyBudget, true) => "orchestrator.end.line.limitDailyMore",
      (RunLimit::GroupBudget, true) => "orchestrator.end.line.limitGroupMore",
      (RunLimit::Tokens, _) => "orchestrator.end.line.limitTokens",
      (RunLimit::Time, _) => "orchestrator.end.line.limitTime",
      (RunLimit::DailyBudget, false) => "orchestrator.end.line.limitDaily",
      (RunLimit::GroupBudget, false) => "orchestrator.end.line.limitGroup",
      (RunLimit::StepBudget { .. }, _) => "orchestrator.end.line.limitStep",
      (RunLimit::PlanLimit { .. }, _) => "orchestrator.end.line.limitPlan",
    },
    (RunEnd::Interrupted, _) => "orchestrator.end.line.interrupted",
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

/// Why a run that succeeded is left to the person, as its line says it.
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
    MergeLeft::DeliveredByQueue => ctx.t("orchestrator.end.mergeLeft.deliveredByQueue"),
  }
}

/// The link to the task's Summary, where the person decides what comes next, in the words of what it does for `end`.
pub(super) fn open_summary(ctx: &mut Ctx, end: RunEnd, project: ProjectId, task: &str) -> Option<LineLink> {
  let label = match end {
    RunEnd::Succeeded => "orchestrator.line.review",
    RunEnd::Limit { .. } => "orchestrator.line.raise",
    RunEnd::Interrupted => "orchestrator.line.continue",
    RunEnd::Failed | RunEnd::Stopped { .. } => "orchestrator.line.summary",
  };
  summary(ctx, label, project, task)
}

/// The link to task `task`'s Summary in project `project`, in the words of `label`.
fn summary(ctx: &mut Ctx, label: &str, project: ProjectId, task: &str) -> Option<LineLink> {
  let key: TaskKey = task.parse().ok()?;
  let navigation = Navigation::of(ctx);
  let route = Route::Project(project, ProjectPage::Task(key, TaskView::of(TaskTab::Summary)));
  Some(LineLink {
    label: ctx.t(label),
    press: Press::Open(navigation.to(route)),
  })
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
