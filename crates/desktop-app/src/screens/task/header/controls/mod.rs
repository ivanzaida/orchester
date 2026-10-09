//! The run controls at the right of the title (`Pause run` `BOdks`, `Cancel run` `AFYgp` in `lUXff`):
//! Run for a draft that has every detail, Start run for a queued task whose run did not start, Pause run
//! while an agent of the run works (not while it only waits for a person), Resume run while it is
//! paused or stopped by the daily budget, Continue run while it is interrupted, Cancel run until it
//! ends. A request shows it is under way on its button, and no second one goes out while it is; a pause
//! stays under way until the run settled at its checkpoint, as the runner records it. A control that
//! applies but cannot be used now stays in place with a lock: why, in short under the controls and in
//! full in its tooltip (no runner is connected, unfinished tasks block the run, its worktree was removed,
//! or the run cannot be read).

mod action;

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::{
  run::{PauseReason, RunLimit, RunStatus},
  task::TaskStatus,
};
use orchester_workspace::tasks::detail::{RunView, TaskDetail};

pub use self::action::{ControlAction, RunControl, control_action};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  message::message_text,
};

/// Why a control cannot be used now: in a few words under the controls, and in full.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reason {
  pub short: Arc<str>,
  pub full: Arc<str>,
}

/// A control, with why it is unavailable when it is.
pub struct Control {
  pub button: ActionButton,
  pub reason: Option<Reason>,
}

/// The control last pressed, with its press's serial: each press counts one more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requested {
  pub control: RunControl,
  pub serial: u64,
}

impl lurq::app::component::DevtoolsInspectable for Requested {}

/// What the controls are drawn from.
pub struct Controls<'a> {
  pub detail: &'a TaskDetail,
  pub online: bool,
  pub action: &'a ControlAction,
  /// The control a request is under way for.
  pub pending: Option<RunControl>,
  /// Where a press records its control, for the frame's notice when the runner refuses it.
  pub requested: &'a Signal<Option<Requested>>,
}

/// The controls that apply to the task now, in order.
pub fn controls(ctx: &Ctx, controls: &Controls) -> Vec<Control> {
  let mut shown = Shown::default();
  let detail = controls.detail;
  if let Some(unreadable) = &detail.latest_unreadable {
    let reason = Reason {
      short: ctx.t("task.reasons.unreadable"),
      full: message_text(ctx, &unreadable.actions()),
    };
    shown.unavailable(cancel_button(ctx), reason);
    return shown.0;
  }
  let Some(run) = &detail.run else {
    without_run(ctx, controls, &mut shown);
    return shown.0;
  };
  if run.run.status().is_terminal() {
    return shown.0;
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
  shown.0
}

/// Run for a draft that has every detail, Retry run for a queued task whose run did not start.
fn without_run(ctx: &Ctx, controls: &Controls, shown: &mut Shown) {
  let task = &controls.detail.task;
  let (id, label, control) = match task.status() {
    TaskStatus::Draft if task.details().missing().is_empty() => {
      ("task-run", "task.controls.run", RunControl::Run(task.id()))
    }
    TaskStatus::Queued => ("task-start-run", "task.controls.retry", RunControl::Start(task.id())),
    _ => return,
  };
  let start = button(ctx, id, ButtonKind::Primary, (label, Icon::Play));
  shown.offer(ctx, controls, start, control, Gate::Work);
}

/// What a control needs besides a free request: a runner, and for work, no unfinished blocking task.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Gate {
  Runner,
  Work,
}

#[derive(Default)]
struct Shown(Vec<Control>);

impl Shown {
  fn unavailable(&mut self, button: ActionButton, reason: Reason) {
    self.0.push(Control {
      button: button.unavailable(reason.full.clone()),
      reason: Some(reason),
    });
  }

  fn push(&mut self, button: ActionButton) {
    self.0.push(Control { button, reason: None });
  }

  /// `button` for `control`: under way while its request is, unavailable while `gate` does not hold
  /// (blocking tasks first: they keep it unavailable with a runner too), pressable otherwise.
  fn offer(&mut self, ctx: &Ctx, controls: &Controls, button: ActionButton, control: RunControl, gate: Gate) {
    if controls.pending == Some(control) {
      self.push(button.loading());
      return;
    }
    let blocked = &controls.detail.blocked_by;
    if gate == Gate::Work && !blocked.is_empty() {
      let tasks = blocked.join(&ctx.t("task.listSeparator"));
      let reason = Reason {
        short: ctx.t_args("task.reasons.blocked", [("tasks", tasks.as_str())]),
        full: ctx.t_args("task.controls.blocked", [("tasks", tasks.as_str())]),
      };
      self.unavailable(button, reason);
      return;
    }
    if !controls.online {
      let reason = Reason {
        short: ctx.t("task.reasons.offline"),
        full: ctx.t("task.controls.offline"),
      };
      self.unavailable(button, reason);
      return;
    }
    if controls.pending.is_some() {
      self.push(button.disabled());
      return;
    }
    let (action, requested) = (controls.action.clone(), controls.requested.clone());
    self.push(button.on_click(move || {
      // One request at a time: a press while one is under way asks nothing.
      if action.state().get_untracked().is_pending() {
        return;
      }
      requested.update(|last| {
        let serial = last.map_or(1, |last| last.serial + 1);
        *last = Some(Requested { control, serial });
      });
      action.run(control);
    }));
  }
}

/// Start, Pause, Resume or Continue for `run`, which has not ended, when one applies.
fn primary(ctx: &Ctx, controls: &Controls, run: &RunView) -> Option<(ActionButton, RunControl, Gate)> {
  let id = run.id();
  let make = |id: &str, kind, label| button(ctx, id, kind, label);
  match run.run.status() {
    RunStatus::Pending if run.first_run => Some((
      make(
        "task-start-run",
        ButtonKind::Primary,
        ("task.controls.start", Icon::Play),
      ),
      RunControl::Start(run.run.task()),
      Gate::Work,
    )),
    RunStatus::Running if run.run.pause_requested() => {
      let pausing = make(
        "task-pause-run",
        ButtonKind::Secondary,
        ("task.controls.pausing", Icon::CirclePause),
      );
      Some((pausing.loading(), RunControl::Pause(id), Gate::Runner))
    }
    // Nothing works while the run only waits for a person: there is nothing to pause.
    RunStatus::Running if run.run.waits_for_a_person() && !controls.detail.lost_its_runner(controls.online) => None,
    RunStatus::Running => Some((
      make(
        "task-pause-run",
        ButtonKind::Secondary,
        ("task.controls.pause", Icon::CirclePause),
      ),
      RunControl::Pause(id),
      Gate::Runner,
    )),
    RunStatus::Paused(PauseReason::Requested | PauseReason::LimitReached(RunLimit::DailyBudget)) => Some((
      make(
        "task-resume-run",
        ButtonKind::Primary,
        ("task.controls.resume", Icon::Play),
      ),
      RunControl::Resume(id),
      Gate::Work,
    )),
    RunStatus::Interrupted(_) => Some((
      make(
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
fn continue_removed(ctx: &Ctx, run: &RunView) -> Option<(ActionButton, Reason)> {
  if !matches!(run.run.status(), RunStatus::Interrupted(_)) {
    return None;
  }
  run.worktree_removed?;
  let number = run.run.number().get().to_string();
  let reason = Reason {
    short: ctx.t("task.reasons.worktreeRemoved"),
    full: ctx.t_args("task.controls.worktreeRemoved", [("run", number.as_str())]),
  };
  let continue_run = button(
    ctx,
    "task-continue-run",
    ButtonKind::Primary,
    ("task.controls.continue", Icon::Play),
  );
  Some((continue_run, reason))
}
