//! What a card of an answer or a message the Orchestrator gives a run's agents says
//! (`docs/design/orchestrator-toolset.md`, "Decisions" 5), in the frame and states of the cancel and merge cards
//! (`b55NqO`, `UKgrR`): its icon and title, where it stands, what it sends or sent (the task's conversation names the
//! Orchestrator as its sender), and the person's action while it waits. No Pencil frame draws these two yet.

use std::sync::Arc;

use lurq::app::ctx::Ctx;

use super::words::{Accept, Phase, View, Words, state};
use crate::ui::{button::ButtonKind, icon::Icon};

/// An answer to the question run `#number` waits with: `answer`, the options picked and the Orchestrator's words.
pub(super) fn answer(ctx: &Ctx, view: &View, (number, answer): (u32, &str)) -> Words {
  let run = number.to_string();
  let args = [("run", run.as_str()), ("task", view.task())];
  let title = match view.phase() {
    Phase::Waits | Phase::Applying => "orchestrator.change.answer.waits",
    Phase::Applied | Phase::Done => "orchestrator.change.answer.done",
    Phase::Dismissed => "orchestrator.change.answer.dismissed",
    Phase::Refused(_) => "orchestrator.change.answer.refused",
  };
  let time = view.time();
  let effect = match view.phase() {
    Phase::Waits | Phase::Applying => ctx.t_args("orchestrator.change.answer.effectWaits", [("answer", answer)]),
    Phase::Applied | Phase::Done => ctx.t_args(
      "orchestrator.change.answer.effectSent",
      [("answer", answer), ("time", time.as_str())],
    ),
    Phase::Dismissed => ctx.t("orchestrator.change.answer.effectDismissed"),
    Phase::Refused(reason) => Arc::from(reason),
  };
  Words {
    glyph: Icon::Reply,
    title: ctx.t_args(title, args),
    auto_mode: false,
    state: state(ctx, view, "orchestrator.change.answer.byYou"),
    effect,
    accept: Some(Accept {
      label: ctx.t("orchestrator.change.answer.accept"),
      glyph: Icon::Reply,
      kind: ButtonKind::Primary,
    }),
    person_only: "orchestrator.change.personOnly.answer",
  }
}

/// A message `text` to the agents of run `#number`.
pub(super) fn message(ctx: &Ctx, view: &View, (number, text): (u32, &str)) -> Words {
  let run = number.to_string();
  let args = [("run", run.as_str()), ("task", view.task())];
  let title = match view.phase() {
    Phase::Waits | Phase::Applying => "orchestrator.change.message.waits",
    Phase::Applied | Phase::Done => "orchestrator.change.message.done",
    Phase::Dismissed => "orchestrator.change.message.dismissed",
    Phase::Refused(_) => "orchestrator.change.message.refused",
  };
  let time = view.time();
  let effect = match view.phase() {
    Phase::Waits | Phase::Applying => ctx.t_args("orchestrator.change.message.effectWaits", [("text", text)]),
    Phase::Applied | Phase::Done => ctx.t_args(
      "orchestrator.change.message.effectSent",
      [("text", text), ("time", time.as_str())],
    ),
    Phase::Dismissed => ctx.t("orchestrator.change.message.effectDismissed"),
    Phase::Refused(reason) => Arc::from(reason),
  };
  Words {
    glyph: Icon::MessageSquareText,
    title: ctx.t_args(title, args),
    auto_mode: false,
    state: state(ctx, view, "orchestrator.change.message.byYou"),
    effect,
    accept: Some(Accept {
      label: ctx.t("orchestrator.change.message.accept"),
      glyph: Icon::MessageSquareText,
      kind: ButtonKind::Primary,
    }),
    person_only: "orchestrator.change.personOnly.message",
  }
}
