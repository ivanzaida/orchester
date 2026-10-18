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
  task::{Task, TaskStatus},
};
use orchester_workspace::tasks::detail::{RunView, TaskDetail};

pub use self::action::{ControlAction, RunControl, control_action, named_steps};
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

/// The page's run controls: its request to the runner and the control last pressed, which the header and
/// the Summary's Start run share, so a press on either shows under way on both and a refusal is said once.
#[derive(Clone)]
pub struct RunControls {
  pub action: ControlAction,
  pub requested: Signal<Option<Requested>>,
}

impl RunControls {
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the task frame provides its run controls before mounting its parts")
  }

  /// The control a request is under way for.
  pub fn pending(&self) -> Option<RunControl> {
    let state = self.action.state().get();
    state
      .is_pending()
      .then_some(self.requested.get())
      .flatten()
      .map(|requested| requested.control)
  }
}

/// What the controls are drawn from.
pub struct Controls<'a> {
  pub detail: &'a TaskDetail,
  pub online: bool,
  pub action: &'a ControlAction,
  /// The control a request is under way for.
  pub pending: Option<RunControl>,
  /// Where a press records its control, for the frame's notice when the runner refuses it.
  pub requested: &'a Signal<Option<Requested>>,
  /// A tool request of the run waits for the person: nothing works, so nothing pauses.
  pub approval_waits: bool,
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

/// Run for a draft that has every detail, Start run for a queued task whose run did not start (`kh5Er` in
/// `Xxrdy`).
fn without_run(ctx: &Ctx, controls: &Controls, shown: &mut Shown) {
  let Some((id, label, control)) = first_run(&controls.detail.task) else {
    return;
  };
  let start = button(ctx, id, ButtonKind::Primary, (label, Icon::Play));
  shown.offer(ctx, controls, start, control, Gate::Work);
}

/// The control that starts a task's first run, its button's id and its label: none for a draft that misses
/// details.
fn first_run(task: &Task) -> Option<(&'static str, &'static str, RunControl)> {
  match task.status() {
    TaskStatus::Draft if task.details().missing().is_empty() => {
      Some(("task-run", "task.controls.run", RunControl::Run(task.id())))
    }
    TaskStatus::Queued => Some(("task-start-run", "task.controls.start", RunControl::Start(task.id()))),
    TaskStatus::Draft => None,
  }
}

/// *Start run* of the Summary before the task's first run (`kh5Er` in `Xxrdy`), as `id`: the header's control
/// for the first run in the design's words, unavailable for the same reasons; none for a draft that misses
/// details.
pub fn first_start(ctx: &Ctx, controls: &Controls, id: &str) -> Option<Control> {
  let (_, _, control) = first_run(&controls.detail.task)?;
  let start = button(ctx, id, ButtonKind::Primary, ("task.controls.start", Icon::Play));
  let mut shown = Shown::default();
  shown.offer(ctx, controls, start, control, Gate::Work);
  shown.0.pop()
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
      let full = if blocked.len() == 1 {
        "task.controls.blocked.one"
      } else {
        "task.controls.blocked.other"
      };
      let reason = Reason {
        short: ctx.t_args("task.reasons.blocked", [("tasks", tasks.as_str())]),
        full: ctx.t_args(full, [("tasks", tasks.as_str())]),
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
    // A run recorded as running without a runner is interrupted: it continues once one is back.
    RunStatus::Running if controls.detail.lost_its_runner(controls.online) => Some((
      make(
        "task-continue-run",
        ButtonKind::Primary,
        ("task.controls.continue", Icon::Play),
      ),
      RunControl::Continue(id),
      Gate::Work,
    )),
    // Nothing works while the run only waits for a person: there is nothing to pause.
    RunStatus::Running if run.run.waits_for_a_person() || controls.approval_waits => None,
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
