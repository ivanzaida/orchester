//! What a card of a decision on how a stopped run goes on, which the Orchestrator's agent proposed
//! (`Orchestrator · continue a stopped run · states` `gQKlW`), says: its icon and title (one more retry, a return
//! to an earlier step, or an agent's resolution of a merge's conflicts), where it stands, its facts (the step it
//! stopped at and why, the choice, the conflicting files, a return's note for its agent), what it would do or did,
//! and the person's action while it waits.

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

/// What kind of continuation a card proposes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
  Retry,
  Return,
  Resolve,
}

impl Kind {
  const fn of(choice: &ContinueChoice) -> Self {
    match choice {
      ContinueChoice::AllowRetry => Self::Retry,
      ContinueChoice::ReturnTo { .. } => Self::Return,
      ContinueChoice::ResolveConflicts { .. } => Self::Resolve,
    }
  }
}

/// Run `#number` goes on with `choice`.
pub(super) fn continuation(ctx: &Ctx, view: &View, (number, choice): (u32, &ContinueChoice)) -> Words {
  let run = number.to_string();
  let step = returned_to(ctx, choice);
  let branch = base_branch(choice);
  let args = [
    ("run", run.as_str()),
    ("task", view.task()),
    ("step", &*step),
    ("branch", branch),
  ];
  let kind = Kind::of(choice);
  let title = match (view.phase(), kind) {
    (Phase::Waits | Phase::Applying, Kind::Retry) => "orchestrator.change.continue.waitsRetry",
    (Phase::Waits | Phase::Applying, Kind::Return) => "orchestrator.change.continue.waitsReturn",
    (Phase::Waits | Phase::Applying, Kind::Resolve) => "orchestrator.change.continue.waitsResolve",
    (Phase::Applied | Phase::Done, Kind::Retry) => "orchestrator.change.continue.doneRetry",
    (Phase::Applied | Phase::Done, Kind::Return) => "orchestrator.change.continue.doneReturn",
    (Phase::Applied | Phase::Done, Kind::Resolve) => "orchestrator.change.continue.doneResolve",
    (Phase::Dismissed, _) => "orchestrator.change.continue.dismissed",
    (Phase::Refused(_), _) => "orchestrator.change.continue.refused",
  };
  let time = view.time();
  let effect = match (view.phase(), kind) {
    (Phase::Waits | Phase::Applying, Kind::Retry) => ctx.t("orchestrator.change.continue.effectWaitsRetry"),
    (Phase::Waits | Phase::Applying, Kind::Return) => {
      ctx.t_args("orchestrator.change.continue.effectWaitsReturn", [("step", &*step)])
    }
    (Phase::Waits | Phase::Applying, Kind::Resolve) => ctx.t_args(
      "orchestrator.change.continue.effectWaitsResolve",
      [("step", &*step), ("branch", branch)],
    ),
    (Phase::Applied | Phase::Done, _) => {
      ctx.t_args("orchestrator.change.continue.effectApplied", [("time", time.as_str())])
    }
    (Phase::Dismissed, Kind::Resolve) => ctx.t("orchestrator.change.continue.effectDismissedResolve"),
    (Phase::Dismissed, _) => ctx.t("orchestrator.change.continue.effectDismissed"),
    (Phase::Refused(reason), _) => Arc::from(reason),
  };
  let (label, glyph) = match kind {
    Kind::Retry => (ctx.t("orchestrator.change.continue.acceptRetry"), Icon::RotateCcw),
    Kind::Return => (
      ctx.t_args("orchestrator.change.continue.acceptReturn", [("step", &*step)]),
      Icon::Undo2,
    ),
    Kind::Resolve => (
      ctx.t_args("orchestrator.change.continue.acceptResolve", [("step", &*step)]),
      Icon::Bot,
    ),
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

/// The facts of a continuation after its run: the step it stopped at and `why`, the choice, the files a resolution
/// resolves, and the note its agent reads.
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
  let returned = returned_to(ctx, choice);
  let chosen = match choice {
    ContinueChoice::AllowRetry => ctx.t("orchestrator.change.facts.choiceRetry"),
    ContinueChoice::ReturnTo { .. } => ctx.t_args("orchestrator.change.facts.choiceReturn", [("step", &*returned)]),
    ContinueChoice::ResolveConflicts { base, .. } => ctx.t_args(
      "orchestrator.change.facts.choiceResolve",
      [("step", &*returned), ("branch", base.as_str())],
    ),
  };
  rows.push((ctx.t("orchestrator.change.facts.choice"), vec![Part::Value(chosen)]));
  match choice {
    ContinueChoice::AllowRetry => {}
    ContinueChoice::ReturnTo { note, .. } => rows.push((
      ctx.t("orchestrator.change.facts.note"),
      vec![Part::Value(note.as_str().into())],
    )),
    ContinueChoice::ResolveConflicts { base, files, note, .. } => {
      let separator = ctx.t("orchestrator.change.facts.separator");
      rows.push((
        ctx.t("orchestrator.change.facts.conflicts"),
        vec![Part::Value(files.join(&separator).into())],
      ));
      // The conflict's own words always lead what the agent reads; the chat's note follows them.
      let mut parts = vec![Part::Value(
        ctx.t_args("orchestrator.change.facts.conflictNote", [("branch", base.as_str())]),
      )];
      parts.extend(note.as_deref().map(|note| Part::Value(note.into())));
      rows.push((ctx.t("orchestrator.change.facts.note"), parts));
    }
  }
  rows
}

/// The step a return goes back to, as people read it (its key while the pipeline no longer lists it); empty for a
/// retry.
fn returned_to(ctx: &Ctx, choice: &ContinueChoice) -> Arc<str> {
  match choice {
    ContinueChoice::AllowRetry => Arc::from(""),
    ContinueChoice::ReturnTo { to: Some(to), .. } | ContinueChoice::ResolveConflicts { to: Some(to), .. } => {
      step_label(ctx, to)
    }
    ContinueChoice::ReturnTo { step, to: None, .. } | ContinueChoice::ResolveConflicts { step, to: None, .. } => {
      Arc::from(step.as_str())
    }
  }
}

/// The branch whose merge a resolution resolves; empty for any other choice.
fn base_branch(choice: &ContinueChoice) -> &str {
  match choice {
    ContinueChoice::ResolveConflicts { base, .. } => base,
    ContinueChoice::AllowRetry | ContinueChoice::ReturnTo { .. } => "",
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
