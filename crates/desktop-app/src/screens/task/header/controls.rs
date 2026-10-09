//! The run controls at the right of the title (`Pause run` `BOdks`, `Cancel run` `AFYgp` in `lUXff`):
//! Start run for a queued task whose run did not start, Pause run while an agent of the run works (not
//! while it only waits for a person), Resume run while it is paused or stopped by the daily budget,
//! Continue run while it is interrupted, Cancel run until it ends. A request shows it is under way on its
//! button; a pause stays under way until the run settled at its checkpoint, as the runner records it.
//! A control that applies but cannot be used now stays in place with a lock, and its reason is said
//! under the controls: no runner is connected, unfinished tasks block the run, its worktree was
//! removed, or the run cannot be read.

use std::sync::Arc;

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_core::{
  id::{RunId, TaskId},
  run::{PauseReason, RunLimit, RunStatus},
  task::TaskStatus,
};
use orchester_ipc::Command;
use orchester_workspace::{
  messages::Message,
  runner::Runners,
  sharing::{Servers, project::WorkClaim},
  tasks::{
    cancel_run::{Stop, stop_named_run},
    detail::{RunView, TaskDetail},
    run::control_message,
  },
  workspace::OpenProject,
};

use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  message::message_text,
};

/// A request about the task's run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunControl {
  /// Start the task's first run, under its first-run key: never a second run.
  Start(TaskId),
  Pause(RunId),
  Resume(RunId),
  /// Continue an interrupted run from its last checkpoint.
  Continue(RunId),
  /// Cancel the run the page shows, and no run that started since.
  Cancel(RunId),
}

impl lurq::app::component::DevtoolsInspectable for RunControl {}

impl RunControl {
  /// The key of what the frame's notice says when the runner did not carry it out.
  pub const fn refused_key(self) -> &'static str {
    match self {
      Self::Start(_) => "task.controls.refused.start",
      Self::Pause(_) => "task.controls.refused.pause",
      Self::Resume(_) => "task.controls.refused.resume",
      Self::Continue(_) => "task.controls.refused.continue",
      Self::Cancel(_) => "task.controls.refused.cancel",
    }
  }
}

pub type ControlAction = FutureAction<RunControl, RunControl, Message>;

/// Asks the runner of `project` to carry out a control of `task`'s run. Work on a shared task goes on
/// only under the person's claim; Pause and Cancel need none.
pub fn control_action(
  ctx: &mut Ctx,
  (project, task): (OpenProject, TaskId),
  (runners, servers): (Runners, Servers),
) -> ControlAction {
  ctx.future_action(move |control: RunControl| {
    let (project, runners, servers) = (project.clone(), runners.clone(), servers.clone());
    async move {
      let claim = match control {
        RunControl::Start(_) | RunControl::Resume(_) | RunControl::Continue(_) => servers
          .claim_for_work(&project.repo_root, task)
          .await
          .map_err(|refusal| refusal.message())?,
        RunControl::Pause(_) | RunControl::Cancel(_) => WorkClaim::default(),
      };
      let result = carry_out(&project, (&runners, task), control).await;
      if result.is_err() {
        claim.release(&servers).await;
      }
      result.map(|()| control)
    }
  })
}

async fn carry_out(
  project: &OpenProject,
  (runners, task): (&Runners, TaskId),
  control: RunControl,
) -> Result<(), Message> {
  let result = match control {
    RunControl::Start(task) => runners
      .start_run_in(project, task.first_run_key(), task)
      .await
      .map(|_| ()),
    RunControl::Pause(run) => runners.control_run_in(project, Command::PauseRun { run }).await,
    RunControl::Resume(run) => runners.control_run_in(project, Command::ResumeRun { run }).await,
    RunControl::Continue(run) => runners.continue_run_in(project, run).await,
    RunControl::Cancel(run) => {
      return stop_named_run(&project.repo_root, task, Stop::Confirmed(Some(run)), runners)
        .await
        .map_err(|refused| refused.message());
    }
  };
  result.map_err(|error| {
    tracing::warn!(%error, ?control, "the runner did not carry out the request");
    control_message(&error)
  })
}

/// What the controls are drawn from.
pub struct Controls<'a> {
  pub detail: &'a TaskDetail,
  pub online: bool,
  pub action: &'a ControlAction,
  /// The control a request is under way for.
  pub pending: Option<RunControl>,
  /// Where a press records its control, for the frame's notice when the runner refuses it.
  pub requested: &'a Signal<Option<RunControl>>,
}

/// The controls that apply to the task now, in order, and why the ones that cannot be used now are
/// unavailable.
pub fn controls(ctx: &Ctx, controls: &Controls) -> (Vec<ActionButton>, Vec<Arc<str>>) {
  let mut shown = Shown::default();
  let detail = controls.detail;
  if let Some(unreadable) = &detail.latest_unreadable {
    let reason = message_text(ctx, &unreadable.actions());
    shown.unavailable(cancel_button(ctx), reason);
    return shown.into_parts();
  }
  let Some(run) = &detail.run else {
    if detail.task.status() == TaskStatus::Queued {
      let start = button(
        ctx,
        "task-start-run",
        ButtonKind::Primary,
        ("task.controls.retry", Icon::Play),
      );
      shown.offer(ctx, controls, start, RunControl::Start(detail.task.id()), Gate::Work);
    }
    return shown.into_parts();
  };
  if run.run.status().is_terminal() {
    return shown.into_parts();
  }
  if let Some((continue_run, reason)) = continue_removed(ctx, run) {
    shown.unavailable(continue_run, reason);
  } else if let Some((start, control, gate)) = primary(ctx, controls, run) {
    shown.offer(ctx, controls, start, control, gate);
  }
  shown.offer(
    ctx,
    controls,
    cancel_button(ctx),
    RunControl::Cancel(run.id()),
    Gate::Runner,
  );
  shown.into_parts()
}

/// What a control needs besides a free request: a runner, and for work, no unfinished blocking task.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Gate {
  Runner,
  Work,
}

#[derive(Default)]
struct Shown {
  buttons: Vec<ActionButton>,
  reasons: Vec<Arc<str>>,
}

impl Shown {
  fn unavailable(&mut self, button: ActionButton, reason: Arc<str>) {
    self.buttons.push(button.unavailable(reason.clone()));
    if !self.reasons.contains(&reason) {
      self.reasons.push(reason);
    }
  }

  /// `button` for `control`: under way while its request is, unavailable while `gate` does not hold,
  /// pressable otherwise.
  fn offer(&mut self, ctx: &Ctx, controls: &Controls, button: ActionButton, control: RunControl, gate: Gate) {
    if controls.pending == Some(control) {
      self.buttons.push(button.loading());
      return;
    }
    if !controls.online {
      self.unavailable(button, ctx.t("task.controls.offline"));
      return;
    }
    let blocked = &controls.detail.blocked_by;
    if gate == Gate::Work && !blocked.is_empty() {
      let tasks = blocked.join(&ctx.t("task.listSeparator"));
      self.unavailable(button, ctx.t_args("task.controls.blocked", [("tasks", tasks.as_str())]));
      return;
    }
    if controls.pending.is_some() {
      self.buttons.push(button.disabled());
      return;
    }
    let (action, requested) = (controls.action.clone(), controls.requested.clone());
    self.buttons.push(button.on_click(move || {
      requested.set(Some(control));
      action.run(control);
    }));
  }

  fn into_parts(self) -> (Vec<ActionButton>, Vec<Arc<str>>) {
    (self.buttons, self.reasons)
  }
}

/// Start, Pause, Resume or Continue for `run`, which has not ended, when one applies.
fn primary(ctx: &Ctx, controls: &Controls, run: &RunView) -> Option<(ActionButton, RunControl, Gate)> {
  let id = run.id();
  let resume = || {
    button(
      ctx,
      "task-resume-run",
      ButtonKind::Primary,
      ("task.controls.resume", Icon::Play),
    )
  };
  match run.run.status() {
    RunStatus::Pending if run.first_run => Some((
      button(
        ctx,
        "task-start-run",
        ButtonKind::Primary,
        ("task.controls.start", Icon::Play),
      ),
      RunControl::Start(run.run.task()),
      Gate::Work,
    )),
    RunStatus::Running if run.run.pause_requested() => {
      let pausing = button(
        ctx,
        "task-pause-run",
        ButtonKind::Secondary,
        ("task.controls.pausing", Icon::CirclePause),
      );
      Some((pausing.loading(), RunControl::Pause(id), Gate::Runner))
    }
    // Nothing works while the run only waits for a person: there is nothing to pause.
    RunStatus::Running if run.run.waits_for_a_person() && !controls.detail.lost_its_runner(controls.online) => None,
    RunStatus::Running => Some((
      button(
        ctx,
        "task-pause-run",
        ButtonKind::Secondary,
        ("task.controls.pause", Icon::CirclePause),
      ),
      RunControl::Pause(id),
      Gate::Runner,
    )),
    RunStatus::Paused(PauseReason::Requested | PauseReason::LimitReached(RunLimit::DailyBudget)) => {
      Some((resume(), RunControl::Resume(id), Gate::Work))
    }
    RunStatus::Interrupted(_) => Some((
      button(
        ctx,
        "task-continue-run",
        ButtonKind::Primary,
        ("task.controls.continue", Icon::Play),
      ),
      RunControl::Continue(id),
      Gate::Work,
    )),
    _ => None,
  }
}

fn button(ctx: &Ctx, id: &str, kind: ButtonKind, (label, glyph): (&str, Icon)) -> ActionButton {
  ActionButton::new(id.to_owned(), kind, ctx.t(label)).icon(glyph)
}

fn cancel_button(ctx: &Ctx) -> ActionButton {
  button(
    ctx,
    "task-cancel-run",
    ButtonKind::Ghost,
    ("task.controls.cancel", Icon::CircleStop),
  )
}

/// Continue run of an interrupted run whose worktree was removed: it stays in its place, unavailable,
/// with why; Run again starts a new run instead.
fn continue_removed(ctx: &Ctx, run: &RunView) -> Option<(ActionButton, Arc<str>)> {
  if !matches!(run.run.status(), RunStatus::Interrupted(_)) {
    return None;
  }
  run.worktree_removed?;
  let number = run.run.number().get().to_string();
  let reason = ctx.t_args("task.controls.worktreeRemoved", [("run", number.as_str())]);
  let continue_run = button(
    ctx,
    "task-continue-run",
    ButtonKind::Primary,
    ("task.controls.continue", Icon::Play),
  );
  Some((continue_run, reason))
}
