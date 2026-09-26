//! Run controls in the task page's topbar (`ctwcG` in 02.10, `ZcYUg` in 02.A, `EGcA7` in 02.L):
//! Pause run while the run works, Resume run while it is paused, Cancel run until it ends. A
//! request shows that it is under way ("Pausing…", "Resuming…", "Cancelling…"); a pause stays
//! "Pausing…" until the run has settled at its checkpoint, as the runner records it.

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::RunId,
  run::{PauseReason, RunStatus},
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
  Pause,
  Resume,
  Cancel,
}

impl lurq::app::component::DevtoolsInspectable for RunControl {}

pub(super) type ControlAction = FutureAction<(RunId, RunControl), RunControl, Message>;

pub(super) fn control_action(ctx: &mut Ctx) -> ControlAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |(run, control): (RunId, RunControl)| {
    let runners = runners.clone();
    async move {
      let command = match control {
        RunControl::Pause => Command::PauseRun { run },
        RunControl::Resume => Command::ResumeRun { run },
        RunControl::Cancel => Command::CancelRun { run },
      };
      runners.control_run(command).await.map(|()| control).map_err(|error| {
        tracing::warn!(%error, ?control, "the runner did not carry out the request");
        control_message(&error)
      })
    }
  })
}

/// The buttons for `run` in its current state. `requested` is the last request made, which is
/// under way while `action` is pending.
pub(super) fn control_buttons(
  ctx: &Ctx,
  run: &RunView,
  action: &ControlAction,
  requested: &Signal<Option<RunControl>>,
) -> Vec<Element> {
  let id = run.id();
  let pending = if action.state().get().is_pending() {
    requested.get()
  } else {
    None
  };
  let button = |label: &str, glyph, element: &'static str| {
    ActionButton::new(ctx.t(label))
      .icon(glyph)
      .id(element)
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
  };
  let press = |control: RunControl| {
    let (action, requested) = (action.clone(), requested.clone());
    move || {
      requested.set(Some(control));
      action.run((id, control));
    }
  };
  let busy = pending.is_some();
  let cancel = if pending == Some(RunControl::Cancel) {
    button("runs.page.cancelling", Icon::Loader, "task-cancel-run").disabled()
  } else {
    button("runs.page.cancel", Icon::Square, "task-cancel-run")
      .on_click(press(RunControl::Cancel))
      .enabled(!busy)
  };
  let status = run.run.status();
  let first = match status {
    RunStatus::Running if run.run.pause_requested() || pending == Some(RunControl::Pause) => {
      Some(button("runs.page.pausing", Icon::Loader, "task-pause-run").disabled())
    }
    RunStatus::Running => Some(
      button("runs.page.pause", Icon::Pause, "task-pause-run")
        .on_click(press(RunControl::Pause))
        .enabled(!busy),
    ),
    RunStatus::Paused(PauseReason::Requested) if pending == Some(RunControl::Resume) => Some(
      button("runs.page.resuming", Icon::Loader, "task-resume-run")
        .primary()
        .disabled(),
    ),
    RunStatus::Paused(PauseReason::Requested) => Some(
      button("runs.page.resume", Icon::Play, "task-resume-run")
        .primary()
        .on_click(press(RunControl::Resume))
        .enabled(!busy),
    ),
    _ => None,
  };
  if status.is_terminal() {
    return Vec::new();
  }
  first
    .into_iter()
    .chain(std::iter::once(cancel))
    .map(|button| button.render(ctx))
    .collect()
}
