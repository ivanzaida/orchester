//! The run controls (ORC-100: the header's one primary action, `Primary action` `xxoqb` in `j8DakG`, and rows of its
//! *More actions* menu `JapSp`): Run for a draft that has every detail, Start run for a queued task whose run did not
//! start, Pause run while an agent of the run works (not while it only waits for a person), Resume run while it is
//! paused or stopped by the daily budget, Continue run while it is interrupted, Cancel run until it ends. A request
//! shows it is under way, and no second one goes out while it is; a pause stays under way until the run settled at
//! its checkpoint, as the runner records it. A control that applies but cannot be used now stays with a lock and says
//! why (no runner is connected, unfinished tasks block the run, its worktree was removed, or the run cannot be read).
//! Each control is what it offers ([`Offer`]); the header draws it as a button or as a menu row ([`control`]).

mod action;
pub mod control;

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::{
  run::{PauseReason, RunLimit, RunStatus},
  task::{Task, TaskStatus},
};
use orchester_workspace::tasks::detail::{RunView, TaskDetail};

pub use self::{
  action::{ControlAction, RunControl, control_action, named_steps},
  control::{Control, Offer, Reason},
};
use crate::ui::{button::ButtonKind, icon::Icon, message::message_text, pressable::Press};

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

/// The controls that apply to the task now.
#[derive(Default)]
pub struct Shown {
  /// The one that moves the run on: Run, Start run, Resume run or Continue run.
  pub moves: Option<Control>,
  /// Pause run, or that it is pausing.
  pub pause: Option<Control>,
  pub cancel: Option<Control>,
}

/// The controls that apply to the task now.
pub fn controls(ctx: &Ctx, controls: &Controls) -> Shown {
  let detail = controls.detail;
  if let Some(unreadable) = &detail.latest_unreadable {
    let reason = Reason {
      short: ctx.t("task.reasons.unreadable"),
      full: message_text(ctx, &unreadable.actions()),
    };
    return Shown {
      cancel: Some(control(ctx, CANCEL, Offer::Unavailable(reason))),
      ..Shown::default()
    };
  }
  let Some(run) = &detail.run else {
    return Shown {
      moves: without_run(ctx, controls),
      ..Shown::default()
    };
  };
  if run.run.status().is_terminal() {
    return Shown::default();
  }
  let mut shown = Shown::default();
  if let Some(reason) = continue_removed(ctx, run) {
    shown.moves = Some(control(ctx, CONTINUE, Offer::Unavailable(reason)));
  } else if *run.run.status() == RunStatus::Running && run.run.pause_requested() {
    // A pause stays under way until the run settled at its checkpoint.
    shown.pause = Some(control(ctx, PAUSING, Offer::Pending));
  } else if let Some((look, run_control, gate)) = primary(controls, run) {
    let offered = offered(ctx, controls, look, run_control, gate);
    if matches!(run_control, RunControl::Pause(_)) {
      shown.pause = Some(offered);
    } else {
      shown.moves = Some(offered);
    }
  }
  let cancel = RunControl::Cancel(run.id());
  shown.cancel = Some(offered(ctx, controls, CANCEL, cancel, Gate::Runner));
  shown
}

/// Run for a draft that has every detail, Start run for a queued task whose run did not start (`kh5Er` in
/// `Xxrdy`).
fn without_run(ctx: &Ctx, controls: &Controls) -> Option<Control> {
  let (look, run_control) = first_run(&controls.detail.task)?;
  Some(offered(ctx, controls, look, run_control, Gate::Work))
}

/// A control's id, words, glyph and look as a button.
type Look = (&'static str, &'static str, Icon, ButtonKind);

const RUN: Look = ("task-run", "task.controls.run", Icon::Play, ButtonKind::Primary);
const START: Look = ("task-start-run", "task.controls.start", Icon::Play, ButtonKind::Primary);
const CONTINUE: Look = (
  "task-continue-run",
  "task.controls.continue",
  Icon::Play,
  ButtonKind::Primary,
);
const RESUME: Look = (
  "task-resume-run",
  "task.controls.resume",
  Icon::Play,
  ButtonKind::Primary,
);
const PAUSE: Look = (
  "task-pause-run",
  "task.controls.pause",
  Icon::CirclePause,
  ButtonKind::Secondary,
);
const PAUSING: Look = (
  "task-pause-run",
  "task.controls.pausing",
  Icon::CirclePause,
  ButtonKind::Secondary,
);
const CANCEL: Look = (
  "task-cancel-run",
  "task.controls.cancel",
  Icon::CircleStop,
  ButtonKind::DangerGhost,
);

/// The control that starts a task's first run: none for a draft that misses details.
fn first_run(task: &Task) -> Option<(Look, RunControl)> {
  match task.status() {
    TaskStatus::Draft if task.details().missing().is_empty() => Some((RUN, RunControl::Run(task.id()))),
    TaskStatus::Queued => Some((START, RunControl::Start(task.id()))),
    TaskStatus::Draft => None,
  }
}

/// *Start run* of the Summary before the task's first run (`kh5Er` in `Xxrdy`), as `id`: the header's control
/// for the first run in the design's words, unavailable for the same reasons; none for a draft that misses
/// details.
pub fn first_start(ctx: &Ctx, controls: &Controls, id: &'static str) -> Option<Control> {
  let (_, run_control) = first_run(&controls.detail.task)?;
  let look = (id, "task.controls.start", Icon::Play, ButtonKind::Primary);
  Some(offered(ctx, controls, look, run_control, Gate::Work))
}

fn control(ctx: &Ctx, (id, label, glyph, kind): Look, offer: Offer) -> Control {
  Control::new(ctx, (id, label, glyph), kind, offer)
}

/// What a control needs besides a free request: a runner, and for work, no unfinished blocking task.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Gate {
  Runner,
  Work,
}

/// The control `look` for `run_control`: under way while its request is, unavailable while `gate` does not hold
/// (blocking tasks first: they keep it unavailable with a runner too), pressable otherwise.
fn offered(ctx: &Ctx, controls: &Controls, look: Look, run_control: RunControl, gate: Gate) -> Control {
  let offer = if controls.pending == Some(run_control) {
    Offer::Pending
  } else if let Some(reason) = gated(ctx, controls, gate) {
    Offer::Unavailable(reason)
  } else if controls.pending.is_some() {
    Offer::Disabled
  } else {
    let (action, requested) = (controls.action.clone(), controls.requested.clone());
    Offer::Ready(Press::run(move || {
      // One request at a time: a press while one is under way asks nothing.
      if action.state().get_untracked().is_pending() {
        return;
      }
      requested.update(|last| {
        let serial = last.map_or(1, |last| last.serial + 1);
        *last = Some(Requested {
          control: run_control,
          serial,
        });
      });
      action.run(run_control);
    }))
  };
  control(ctx, look, offer)
}

/// Why `gate` does not hold now, if it does not.
fn gated(ctx: &Ctx, controls: &Controls, gate: Gate) -> Option<Reason> {
  let blocked = &controls.detail.blocked_by;
  if gate == Gate::Work && !blocked.is_empty() {
    let tasks = blocked.join(&ctx.t("task.listSeparator"));
    let full = if blocked.len() == 1 {
      "task.controls.blocked.one"
    } else {
      "task.controls.blocked.other"
    };
    return Some(Reason {
      short: ctx.t_args("task.reasons.blocked", [("tasks", tasks.as_str())]),
      full: ctx.t_args(full, [("tasks", tasks.as_str())]),
    });
  }
  (!controls.online).then(|| Reason {
    short: ctx.t("task.reasons.offline"),
    full: ctx.t("task.controls.offline"),
  })
}

/// Start, Pause, Resume or Continue for `run`, which has not ended and is not pausing, when one applies.
fn primary(controls: &Controls, run: &RunView) -> Option<(Look, RunControl, Gate)> {
  let id = run.id();
  match run.run.status() {
    RunStatus::Pending if run.first_run => Some((START, RunControl::Start(run.run.task()), Gate::Work)),
    // A run recorded as running without a runner is interrupted: it continues once one is back.
    RunStatus::Running if controls.detail.lost_its_runner(controls.online) => {
      Some((CONTINUE, RunControl::Continue(id), Gate::Work))
    }
    // Nothing works while the run only waits for a person: there is nothing to pause.
    RunStatus::Running if run.run.waits_for_a_person() || controls.approval_waits => None,
    RunStatus::Running => Some((PAUSE, RunControl::Pause(id), Gate::Runner)),
    RunStatus::Paused(PauseReason::Requested | PauseReason::LimitReached(RunLimit::DailyBudget)) => {
      Some((RESUME, RunControl::Resume(id), Gate::Work))
    }
    RunStatus::Interrupted(_) => Some((CONTINUE, RunControl::Continue(id), Gate::Work)),
    _ => None,
  }
}

/// Why Continue run of an interrupted run whose worktree was removed is unavailable: it stays, with why; Run again
/// starts a new run instead. A removal that left the folder on disk says it is not removed (ORC-418).
fn continue_removed(ctx: &Ctx, run: &RunView) -> Option<Reason> {
  if !matches!(run.run.status(), RunStatus::Interrupted(_)) {
    return None;
  }
  run.worktree_removed?;
  let number = run.run.number().get().to_string();
  let run_arg = ("run", number.as_str());
  Some(match &run.worktree_not_removed {
    Some(left) => Reason {
      short: ctx.t("task.reasons.worktreeNotRemoved"),
      full: ctx.t_args(
        "task.controls.worktreeNotRemoved",
        [run_arg, ("path", left.path.as_str())],
      ),
    },
    None => Reason {
      short: ctx.t("task.reasons.worktreeRemoved"),
      full: ctx.t_args("task.controls.worktreeRemoved", [run_arg]),
    },
  })
}
