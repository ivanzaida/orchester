//! What a card of a pause, a resume or a release of new starts says, which the Orchestrator's agent proposed in a turn
//! a wait or a run's end started by itself (`docs/design/orchestrator-toolset.md`, decision 5), in the frame and states
//! of the cancel and merge cards (`Ee2JE`): its icon and title, where it stands, what it would do or did, and the
//! person's action while it waits. No Pencil frame draws these three yet.

use std::sync::Arc;

use lurq::app::ctx::Ctx;

use super::words::{Accept, Phase, View, Words, state};
use crate::ui::{button::ButtonKind, icon::Icon};

/// Which of the three a card is.
#[derive(Clone, Copy)]
pub(super) enum Control {
  Pause,
  Resume,
  Release,
}

impl Control {
  /// The area of its words (`orchestrator.change.<area>.*`) and its icon.
  const fn look(self) -> (&'static str, Icon) {
    match self {
      Self::Pause => ("pause", Icon::CirclePause),
      Self::Resume => ("resume", Icon::Play),
      Self::Release => ("release", Icon::Play),
    }
  }
}

/// The card of `control` of run `#number` (none for a release), whose group task `group` names when it names one.
pub(super) fn control(ctx: &Ctx, view: &View, (control, number, group): (Control, Option<u32>, Option<&str>)) -> Words {
  let (area, glyph) = control.look();
  let key = |leaf: &str| format!("orchestrator.change.{area}.{leaf}");
  let run = number.map(|number| number.to_string()).unwrap_or_default();
  let args = [
    ("run", run.as_str()),
    ("task", view.task()),
    ("group", group.unwrap_or_default()),
  ];
  let title = match (view.phase(), group.is_some()) {
    (Phase::Waits | Phase::Applying, false) => key("waits"),
    (Phase::Waits | Phase::Applying, true) => key("waitsGroup"),
    (Phase::Applied | Phase::Done, false) => key("done"),
    (Phase::Applied | Phase::Done, true) => key("doneGroup"),
    (Phase::Dismissed, _) => key("dismissed"),
    (Phase::Refused(_), _) => key("refused"),
  };
  let time = view.time();
  let effect = match view.phase() {
    Phase::Waits | Phase::Applying => ctx.t(&key("effectWaits")),
    Phase::Applied | Phase::Done => ctx.t_args(&key("effectApplied"), [("time", time.as_str())]),
    Phase::Dismissed => ctx.t(&key("effectDismissed")),
    Phase::Refused(reason) => Arc::from(reason),
  };
  Words {
    glyph,
    title: ctx.t_args(&title, args),
    auto_mode: false,
    state: state(ctx, view, by_you(control)),
    effect,
    accept: Some(Accept {
      label: ctx.t(&key("accept")),
      glyph,
      kind: ButtonKind::Primary,
    }),
    person_only: person_only(control),
  }
}

/// The state of an applied card.
const fn by_you(control: Control) -> &'static str {
  match control {
    Control::Pause => "orchestrator.change.pause.byYou",
    Control::Resume => "orchestrator.change.resume.byYou",
    Control::Release => "orchestrator.change.release.byYou",
  }
}

/// What a refused click by automation says.
const fn person_only(control: Control) -> &'static str {
  match control {
    Control::Pause => "orchestrator.change.personOnly.pause",
    Control::Resume => "orchestrator.change.personOnly.resume",
    Control::Release => "orchestrator.change.personOnly.release",
  }
}
