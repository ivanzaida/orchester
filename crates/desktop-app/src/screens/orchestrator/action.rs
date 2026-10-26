//! An action the Orchestrator took through one of Orchester's tools, in its answer, as an event line (`ACTIONS THE
//! ORCHESTRATOR TOOK` in `1 · Event lines` `VGbU7`): done (`circle-check` in `success`, what was done and the task it
//! names, *Open ↗* the task), refused (`ban` in `danger`, *… · not done*, the tool's own reason on hover and once
//! pressed, *Open ↗* the task, or the queue for a refused hold or release of new starts), dismissed (`x` in
//! `text_muted`), or a proposal that waits without its card. Several in one answer are one list
//! ([`super::line::list`]).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  id::ProjectId,
  orchestrator::{ActionOutcome, ChatAction},
  project::TaskKey,
  time::Timestamp,
};
use orchester_workspace::display;

use super::line::{Chip, ChipTone, Line, LineLink, Tone};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList, TaskView},
  ui::{icon::Icon, pressable::Press},
};

/// What the line says of a tool, by its name: the keys of what it did and of its refusal.
fn words(tool: &str) -> (&'static str, &'static str) {
  match tool {
    "create_task" => (
      "orchestrator.action.createTask.done",
      "orchestrator.action.createTask.refused",
    ),
    "update_task" => (
      "orchestrator.action.updateTask.done",
      "orchestrator.action.updateTask.refused",
    ),
    "run_task" => (
      "orchestrator.action.runTask.done",
      "orchestrator.action.runTask.refused",
    ),
    "pause_run" => (
      "orchestrator.action.pauseRun.done",
      "orchestrator.action.pauseRun.refused",
    ),
    "resume_run" => (
      "orchestrator.action.resumeRun.done",
      "orchestrator.action.resumeRun.refused",
    ),
    "hold_queue" => (
      "orchestrator.action.holdQueue.done",
      "orchestrator.action.holdQueue.refused",
    ),
    "duplicate_pipeline" => (
      "orchestrator.action.duplicatePipeline.done",
      "orchestrator.action.duplicatePipeline.refused",
    ),
    "release_queue" => (
      "orchestrator.action.releaseQueue.done",
      "orchestrator.action.releaseQueue.refused",
    ),
    _ => ("orchestrator.action.other.done", "orchestrator.action.other.refused"),
  }
}

/// The line of `action`, taken at `at` in project `project`; `id` names it.
pub(super) fn line(ctx: &mut Ctx, id: &str, (action, at): (&ChatAction, Timestamp), project: ProjectId) -> Element {
  let (done, refused) = words(&action.tool);
  let task = action.task.as_deref().unwrap_or_default();
  let args = [("task", task), ("tool", action.tool.as_str())];
  let time = display::clock(action.settled.unwrap_or(at));
  let navigation = Navigation::of(ctx);
  let open = task_route(project, task).map(|route| LineLink {
    label: ctx.t("orchestrator.line.open"),
    press: Press::Open(navigation.to(route)),
  });
  let target = (!task.is_empty()).then(|| -> Arc<str> {
    match action.approved_in {
      Some(_) => ctx.t_args(
        "orchestrator.line.targetBy",
        [("target", task), ("by", &*ctx.t("orchestrator.line.byYouInChat"))],
      ),
      None => task.into(),
    }
  });
  let line = match &action.outcome {
    ActionOutcome::Done | ActionOutcome::Applied => {
      Line::new(id, (Icon::CircleCheck, Tone::Success), ctx.t_args(done, args)).target(target)
    }
    ActionOutcome::Refused { reason, .. } => {
      let queue = matches!(action.tool.as_str(), "hold_queue" | "release_queue").then(|| LineLink {
        label: ctx.t("orchestrator.line.queue"),
        press: Press::Open(navigation.to(queue_route(project))),
      });
      return Line::new(id, (Icon::Ban, Tone::Danger), ctx.t_args(refused, args))
        .outcome("refused")
        .time(Some(time))
        .link(queue.or(open))
        .detail(Some(Arc::from(reason.as_str())))
        .render(ctx);
    }
    // Shown with its Start and Dismiss in a kept conversation (`super::proposal`); this is its line without them.
    ActionOutcome::Proposed(start) => Line::new(
      id,
      (Icon::Hourglass, Tone::Warning),
      ctx.t_args(
        "orchestrator.proposal.title",
        [("task", task), ("pipeline", start.pipeline.as_str())],
      ),
    )
    .chip(Some(Chip::new(ctx.t("orchestrator.wait.status.waits"), ChipTone::Warning))),
    // A change waits with its own card (`super::change`); this is a waiting action without it.
    ActionOutcome::Waits => Line::new(id, (Icon::Hourglass, Tone::Warning), ctx.t_args(done, args))
      .target(target)
      .chip(Some(Chip::new(ctx.t("orchestrator.wait.status.waits"), ChipTone::Warning))),
    ActionOutcome::Dismissed => Line::new(
      id,
      (Icon::X, Tone::Muted),
      ctx.t_args("orchestrator.proposal.dismissedTitle", args),
    ),
  };
  line
    .outcome(outcome_name(&action.outcome))
    .time(Some(time))
    .link(open)
    .render(ctx)
}

/// Tasks, where new starts of project `project` are held and released.
pub(super) fn queue_route(project: ProjectId) -> Route {
  Route::Project(project, ProjectPage::Tasks(TaskList::default()))
}

/// The page of task `task` in project `project`, when it is a key people can read.
pub(super) fn task_route(project: ProjectId, task: &str) -> Option<Route> {
  let key: TaskKey = task.parse().ok()?;
  Some(Route::Project(project, ProjectPage::Task(key, TaskView::default())))
}

const fn outcome_name(outcome: &ActionOutcome) -> &'static str {
  match outcome {
    ActionOutcome::Done => "done",
    ActionOutcome::Applied => "applied",
    ActionOutcome::Refused { .. } => "refused",
    ActionOutcome::Proposed(_) | ActionOutcome::Waits => "proposed",
    ActionOutcome::Dismissed => "dismissed",
  }
}
