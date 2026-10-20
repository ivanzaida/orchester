//! What a card of a decision on how a stopped run goes on, which the Orchestrator's agent proposed
//! (`Orchestrator · continue a stopped run · states` `gQKlW`), says: its icon and title (one more retry, or a return
//! to an earlier step), where it stands, its facts (the step it stopped at and why, the choice, a return's note for
//! its agent), what it would do or did, and the person's action while it waits.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::orchestrator::{
  StopReason,
  change::{ContinueChoice, StepName},
};

use super::{
  facts::Part,
  words::{Accept, Phase, View, Words, state},
};
use crate::ui::{button::ButtonKind, icon::Icon};

/// Run `#number` goes on with `choice`.
pub(super) fn continuation(ctx: &Ctx, view: &View, (number, choice): (u32, &ContinueChoice)) -> Words {
  let run = number.to_string();
  let step = returned_to(ctx, choice);
  let args = [("run", run.as_str()), ("task", view.task()), ("step", &*step)];
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
      ctx.t_args("orchestrator.change.continue.effectWaitsReturn", [("step", &*step)])
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
      ctx.t_args("orchestrator.change.continue.acceptReturn", [("step", &*step)]),
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

/// The facts of a continuation after its run: the step it stopped at and `why`, the choice, and a return's note.
pub(super) fn rows(
  ctx: &Ctx,
  (step, why): (Option<&StepName>, Option<StopReason>),
  choice: &ContinueChoice,
) -> Vec<(Arc<str>, Vec<Part>)> {
  let mut rows = Vec::new();
  if let Some(step) = step {
    let mut parts = vec![Part::Value(step_label(ctx, step))];
    parts.extend(why.map(|why| Part::Aside(why_words(ctx, why))));
    rows.push((ctx.t("orchestrator.change.facts.stopped"), parts));
  }
  let chosen = match choice {
    ContinueChoice::AllowRetry => ctx.t("orchestrator.change.facts.choiceRetry"),
    ContinueChoice::ReturnTo { .. } => ctx.t_args(
      "orchestrator.change.facts.choiceReturn",
      [("step", &*returned_to(ctx, choice))],
    ),
  };
  rows.push((ctx.t("orchestrator.change.facts.choice"), vec![Part::Value(chosen)]));
  if let ContinueChoice::ReturnTo { note, .. } = choice {
    rows.push((
      ctx.t("orchestrator.change.facts.note"),
      vec![Part::Value(note.as_str().into())],
    ));
  }
  rows
}

/// The step a return goes back to, as people read it (its key while the pipeline no longer lists it); empty for a
/// retry.
fn returned_to(ctx: &Ctx, choice: &ContinueChoice) -> Arc<str> {
  match choice {
    ContinueChoice::AllowRetry => Arc::from(""),
    ContinueChoice::ReturnTo { to: Some(to), .. } => step_label(ctx, to),
    ContinueChoice::ReturnTo { step, to: None, .. } => Arc::from(step.as_str()),
  }
}

/// `step` with its place in the pipeline (`03 Implement`).
pub(super) fn step_label(ctx: &Ctx, step: &StepName) -> Arc<str> {
  ctx.t_args(
    "orchestrator.change.facts.stepLabel",
    [("number", format!("{:02}", step.place)), ("step", step.name.clone())],
  )
}

/// Why a run waits for a person's decision.
fn why_words(ctx: &Ctx, why: StopReason) -> Arc<str> {
  match why {
    StopReason::RetriesExhausted { retries: 1 } => ctx.t("orchestrator.change.facts.why.retriesOne"),
    StopReason::RetriesExhausted { retries } => ctx.t_args(
      "orchestrator.change.facts.why.retries",
      [("count", retries.to_string())],
    ),
    StopReason::BrokeAgain => ctx.t("orchestrator.change.facts.why.brokeAgain"),
    StopReason::Unverified => ctx.t("orchestrator.change.facts.why.unverified"),
    StopReason::Unchanged => ctx.t("orchestrator.change.facts.why.unchanged"),
  }
}
