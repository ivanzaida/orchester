//! What a card of a decision on how a stopped run goes on, which the Orchestrator's agent proposed
//! (`docs/design/orchestrator-toolset.md`, "Decisions" 5), says, in the frame and states of the cancel and merge cards
//! (`Ee2JE`): its icon and title (one more retry, or a return to an earlier step), where it stands, what it would do
//! or did, and the person's action while it waits. No Pencil frame draws it yet.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::orchestrator::change::ContinueChoice;

use super::words::{Accept, Phase, View, Words, state};
use crate::ui::{button::ButtonKind, icon::Icon};

/// Run `#number` goes on with `choice`.
pub(super) fn continuation(ctx: &Ctx, view: &View, (number, choice): (u32, &ContinueChoice)) -> Words {
  let run = number.to_string();
  let step = match choice {
    ContinueChoice::AllowRetry => "",
    ContinueChoice::ReturnTo { name, .. } => name.as_str(),
  };
  let args = [("run", run.as_str()), ("task", view.task()), ("step", step)];
  let retry = matches!(choice, ContinueChoice::AllowRetry);
  let title = match (view.phase(), retry) {
    (Phase::Waits | Phase::Applying, true) => "orchestrator.change.continue.waitsRetry",
    (Phase::Waits | Phase::Applying, false) => "orchestrator.change.continue.waitsReturn",
    (Phase::Applied | Phase::Done, true) => "orchestrator.change.continue.doneRetry",
    (Phase::Applied | Phase::Done, false) => "orchestrator.change.continue.doneReturn",
    (Phase::Dismissed, _) => "orchestrator.change.continue.dismissed",
    (Phase::Refused(_), _) => "orchestrator.change.continue.refused",
  };
  let time = view.time();
  let effect = match (view.phase(), retry) {
    (Phase::Waits | Phase::Applying, true) => ctx.t("orchestrator.change.continue.effectWaitsRetry"),
    (Phase::Waits | Phase::Applying, false) => {
      ctx.t_args("orchestrator.change.continue.effectWaitsReturn", [("step", step)])
    }
    (Phase::Applied | Phase::Done, _) => {
      ctx.t_args("orchestrator.change.continue.effectApplied", [("time", time.as_str())])
    }
    (Phase::Dismissed, _) => ctx.t("orchestrator.change.continue.effectDismissed"),
    (Phase::Refused(reason), _) => Arc::from(reason),
  };
  let (label, glyph) = if retry {
    (ctx.t("orchestrator.change.continue.acceptRetry"), Icon::RotateCcw)
  } else {
    (
      ctx.t_args("orchestrator.change.continue.acceptReturn", [("step", step)]),
      Icon::Undo2,
    )
  };
  Words {
    glyph,
    title: ctx.t_args(title, args),
    auto_mode: false,
    state: state(ctx, view, "orchestrator.change.continue.byYou"),
    effect,
    accept: Some(Accept {
      label,
      glyph,
      kind: ButtonKind::Primary,
    }),
    person_only: "orchestrator.change.personOnly.continue",
  }
}
