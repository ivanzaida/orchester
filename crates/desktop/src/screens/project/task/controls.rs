//! Run controls in the task page's topbar (`ctwcG` in 02.10, `ZcYUg` in 02.A, `EGcA7` in 02.L,
//! `Oc7iw` in 02.11): Pause run while an agent of the run works (not while it only waits for a
//! decision), Resume run while it is paused or stopped by the daily budget,
//! Continue run while it is interrupted (the task-level continue after a session ended, which
//! `o8DCd` leaves undesigned; it takes Resume's place and look), Cancel run until it ends. A
//! request shows that it is under way ("Pausing…", "Resuming…", "Continuing…", "Cancelling…");
//! a pause stays "Pausing…" until the run has settled at its checkpoint, as the runner records it.

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, TaskId},
  run::{PauseReason, RunLimit, RunStatus},
};
use orchester_ipc::Command;

use super::super::page;
use crate::{
  messages::Message,
  runner::Runners,
  tasks::{detail::RunView, run::control_message},
  ui::{button::ActionButton, icon::Icon},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RunControl {
  StartFirst(TaskId),
  Pause,
  Resume,
  /// Continue an interrupted run from its last checkpoint.
  Continue,
  Cancel,
}

impl lurq::app::component::DevtoolsInspectable for RunControl {}

pub(super) type ControlAction = FutureAction<(RunId, RunControl), RunControl, Message>;

pub(super) fn control_action(ctx: &mut Ctx) -> ControlAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  let servers = crate::sharing::feed::ServerFeed::of(ctx).servers;
  let props = ctx.props::<super::TaskProps>();
  let (root, page_task) = (props.project.repo_root.clone(), props.task);
  ctx.future_action(move |(run, control): (RunId, RunControl)| {
    let (runners, servers, root) = (runners.clone(), servers.clone(), root.clone());
    async move {
      // Work on a shared task goes on only under the person's claim; Pause and Cancel need none.
      let works = match control {
        RunControl::StartFirst(task) => Some(task),
        RunControl::Resume | RunControl::Continue => Some(page_task),
        RunControl::Pause | RunControl::Cancel => None,
      };
      let claim = match works {
        Some(task) => servers
          .claim_for_work(&root, task)
          .await
          .map_err(|refusal| refusal.message())?,
        None => crate::sharing::project::WorkClaim::default(),
      };
      let result = match control {
        RunControl::StartFirst(task) => runners.start_run(task.first_run_key(), task).await.map(|_| ()),
        RunControl::Pause => runners.control_run(Command::PauseRun { run }).await,
        RunControl::Resume => runners.control_run(Command::ResumeRun { run }).await,
        RunControl::Continue => runners.continue_run(run).await,
        RunControl::Cancel => runners.control_run(Command::CancelRun { run }).await,
      };
      if result.is_err() {
        claim.release(&servers).await;
      }
      result.map(|()| control).map_err(|error| {
        tracing::warn!(%error, ?control, "the runner did not carry out the request");
        control_message(&error)
      })
    }
  })
}

/// The buttons for `run` in its current state. `requested` is the last request made, which is
/// under way while `action` is pending. While the task is `blocked` by unfinished tasks, Start,
/// Resume and Continue are unavailable; the page's Blocked notice says why.
pub(super) fn control_buttons(
  ctx: &Ctx,
  run: &RunView,
  (action, blocked): (&ControlAction, bool),
  requested: &Signal<Option<RunControl>>,
) -> Vec<Element> {
  let pending = if action.state().get().is_pending() {
    requested.get()
  } else {
    None
  };
  let buttons = ControlButtons {
    ctx,
    run,
    action,
    requested,
    pending,
    blocked,
  };
  if run.run.status().is_terminal() {
    return Vec::new();
  }
  buttons
    .first()
    .into_iter()
    .chain(std::iter::once(buttons.cancel()))
    .map(|button| button.render(ctx))
    .collect()
}

struct ControlButtons<'a> {
  ctx: &'a Ctx,
  run: &'a RunView,
  action: &'a ControlAction,
  requested: &'a Signal<Option<RunControl>>,
  pending: Option<RunControl>,
  /// Unfinished blocking tasks keep the run from starting or going on.
  blocked: bool,
}

impl ControlButtons<'_> {
  fn button(&self, label: &str, glyph: Icon, id: &'static str) -> ActionButton {
    ActionButton::new(self.ctx.t(label))
      .icon(glyph)
      .id(id)
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
  }

  fn press(&self, control: RunControl) -> impl Fn() + Send + Sync + 'static {
    let (action, requested, id) = (self.action.clone(), self.requested.clone(), self.run.id());
    move || {
      requested.set(Some(control));
      action.run((id, control));
    }
  }

  fn cancel(&self) -> ActionButton {
    if self.pending == Some(RunControl::Cancel) {
      self
        .button("runs.page.cancelling", Icon::Loader, "task-cancel-run")
        .disabled()
    } else {
      self
        .button("runs.page.cancel", Icon::Square, "task-cancel-run")
        .on_click(self.press(RunControl::Cancel))
        .enabled(self.pending.is_none())
    }
  }

  fn first(&self) -> Option<ActionButton> {
    if self.run.run.status() == &RunStatus::Pending && self.run.first_run {
      return Some(if self.pending.is_some() {
        self
          .button("runs.page.starting", Icon::Loader, "task-start-pending-run")
          .primary()
          .disabled()
      } else {
        self
          .button("runs.page.start", Icon::Play, "task-start-pending-run")
          .primary()
          .on_click(self.press(RunControl::StartFirst(self.run.run.task())))
          .enabled(!self.blocked)
      });
    }
    match self.run.run.status() {
      RunStatus::Running if self.run.run.pause_requested() || self.pending == Some(RunControl::Pause) => Some(
        self
          .button("runs.page.pausing", Icon::Loader, "task-pause-run")
          .disabled(),
      ),
      // Nothing works while the run only waits for a person: there is nothing to pause.
      RunStatus::Running if self.run.run.waits_for_a_person() => None,
      RunStatus::Running => Some(
        self
          .button("runs.page.pause", Icon::Pause, "task-pause-run")
          .on_click(self.press(RunControl::Pause))
          .enabled(self.pending.is_none()),
      ),
      RunStatus::Paused(PauseReason::Requested | PauseReason::LimitReached(RunLimit::DailyBudget))
        if self.pending == Some(RunControl::Resume) =>
      {
        Some(
          self
            .button("runs.page.resuming", Icon::Loader, "task-resume-run")
            .primary()
            .disabled(),
        )
      }
      RunStatus::Paused(PauseReason::Requested | PauseReason::LimitReached(RunLimit::DailyBudget)) => Some(
        self
          .button("runs.page.resume", Icon::Play, "task-resume-run")
          .primary()
          .on_click(self.press(RunControl::Resume))
          .enabled(self.pending.is_none() && !self.blocked),
      ),
      // Its worktree was removed: it stays in its place, unavailable, and the lock line under the
      // notice says why (63.05 `LmTnc`).
      RunStatus::Interrupted(_) if self.run.worktree_removed.is_some() => {
        Some(self.button("runs.page.continue", Icon::Play, "task-continue-run"))
      }
      RunStatus::Interrupted(_) if self.pending == Some(RunControl::Continue) => Some(
        self
          .button("runs.page.continuing", Icon::Loader, "task-continue-run")
          .primary()
          .disabled(),
      ),
      RunStatus::Interrupted(_) => Some(
        self
          .button("runs.page.continue", Icon::Play, "task-continue-run")
          .primary()
          .on_click(self.press(RunControl::Continue))
          .enabled(self.pending.is_none() && !self.blocked),
      ),
      _ => None,
    }
  }
}
