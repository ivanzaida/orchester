//! What a card of an answer or a message the Orchestrator gives a run's agents says
//! (`docs/design/orchestrator-toolset.md`, "Decisions" 5), in the frame and states of the cancel and merge cards
//! (`b55NqO`, `UKgrR`): its icon and title, where it stands, what it sends or sent (the task's conversation names the
//! Orchestrator as its sender) and, for a message to a run none of whose agents waits or works, how sending it moves
//! the run, and the person's action while it waits. No Pencil frame draws these two yet.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::orchestrator::change::MessageThen;

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
    chip: None,
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

/// A message `text` to the agents of run `#number`, which moves the run as `then` says when none of its agents waits
/// or works.
pub(super) fn message(ctx: &Ctx, view: &View, (number, text, then): (u32, &str, Option<MessageThen>)) -> Words {
  // A message that ran the task again reads so as soon as the card keeps the run it started (`ZvQqc`); the run's
  // number is named once its facts are read.
  let ran_again = view.action.started.is_some() && then == Some(MessageThen::RunAgain);
  let started = view
    .run()
    .filter(|_| ran_again)
    .map(|started| started.number.to_string());
  let run = started.clone().unwrap_or_else(|| number.to_string());
  let args = [("run", run.as_str()), ("task", view.task())];
  let title = match (view.phase(), ran_again, &started) {
    (Phase::Waits | Phase::Applying, ..) => "orchestrator.change.message.waits",
    (Phase::Applied | Phase::Done, true, Some(_)) => "orchestrator.change.message.doneAgain",
    (Phase::Applied | Phase::Done, true, None) => "orchestrator.change.message.doneAgainUnread",
    (Phase::Applied | Phase::Done, false, _) => "orchestrator.change.message.done",
    (Phase::Dismissed, ..) => "orchestrator.change.message.dismissed",
    (Phase::Refused(_), ..) => "orchestrator.change.message.refused",
  };
  let time = view.time();
  let effect = match (view.phase(), ran_again, &started) {
    (Phase::Waits | Phase::Applying, ..) => ctx.t_args(moving(then).0, [("text", text)]),
    (Phase::Applied | Phase::Done, true, Some(started)) => ctx.t_args(
      "orchestrator.change.message.effectRanAgain",
      [("run", started.as_str()), ("text", text), ("time", time.as_str())],
    ),
    (Phase::Applied | Phase::Done, true, None) => ctx.t_args(
      "orchestrator.change.message.effectRanAgainUnread",
      [("text", text), ("time", time.as_str())],
    ),
    (Phase::Applied | Phase::Done, false, _) => ctx.t_args(
      "orchestrator.change.message.effectSent",
      [("text", text), ("time", time.as_str())],
    ),
    (Phase::Dismissed, ..) => ctx.t("orchestrator.change.message.effectDismissed"),
    (Phase::Refused(reason), ..) => Arc::from(reason),
  };
  Words {
    glyph: Icon::MessageSquareText,
    title: ctx.t_args(title, args),
    chip: None,
    state: state(ctx, view, "orchestrator.change.message.byYou"),
    effect,
    accept: Some(Accept {
      label: ctx.t(moving(then).1),
      glyph: Icon::MessageSquareText,
      kind: ButtonKind::Primary,
    }),
    person_only: "orchestrator.change.personOnly.message",
  }
}

/// What a waiting message card says it does, and its action's words: as `then` moves the run, or a plain message. The
/// task's conversation names a waiting message proposal's action with the same words.
pub(in crate::screens) const fn moving(then: Option<MessageThen>) -> (&'static str, &'static str) {
  match then {
    None => (
      "orchestrator.change.message.effectWaits",
      "orchestrator.change.message.accept",
    ),
    Some(MessageThen::FollowUp) => (
      "orchestrator.change.message.then.followUp",
      "orchestrator.change.message.then.acceptFollowUp",
    ),
    Some(MessageThen::Continue) => (
      "orchestrator.change.message.then.continue",
      "orchestrator.change.message.then.acceptContinue",
    ),
    Some(MessageThen::Resume) => (
      "orchestrator.change.message.then.resume",
      "orchestrator.change.message.then.acceptResume",
    ),
    Some(MessageThen::ContinueInterrupted) => (
      "orchestrator.change.message.then.continueInterrupted",
      "orchestrator.change.message.then.acceptContinueInterrupted",
    ),
    Some(MessageThen::RaiseLimit) => (
      "orchestrator.change.message.then.raiseLimit",
      "orchestrator.change.message.accept",
    ),
    Some(MessageThen::RunAgain) => (
      "orchestrator.change.message.then.runAgain",
      "orchestrator.change.message.then.acceptRunAgain",
    ),
  }
}
