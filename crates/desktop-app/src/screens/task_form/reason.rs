//! The one line at the action bar's start (`Reason` `z9ZYc`, `J8TZyN`, `y8uD7`): the single reason the next action
//! cannot be pressed yet, or what will happen once it is. Its icon says which: a lock while an action is held back, a
//! check once the task is ready to run, a loader while an action is under way, a `warning` mark for what went wrong.
//! What a run needs beyond Create is said after it in one sentence: *To create, add a title. To run, also the outcome,
//! one criterion and a pipeline.*

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::resource::Deliverable;

use super::{
  conflict::field_key,
  delivers::delivered,
  state::{Need, Phase, Subject, View},
};
use crate::ui::{icon::Icon, message::message_text};

/// What the line says, and how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reason {
  pub kind: Kind,
  pub words: Arc<str>,
}

/// What kind of line it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  /// An action is held back for what it needs.
  Blocked,
  /// The task is ready to run.
  Ready,
  /// An action is under way.
  Busy,
  /// What went wrong, e.g. the runner's refusal.
  Problem,
  /// What the person should know: kept changes, Retry run.
  Note(Icon),
}

impl Kind {
  pub const fn look(self) -> (Icon, PaletteColor, &'static str) {
    match self {
      Self::Blocked => (Icon::Lock, PaletteColor::TextMuted, "unavailable-reason"),
      Self::Ready => (Icon::CircleCheck, PaletteColor::TextMuted, "ready"),
      Self::Busy => (Icon::LoaderCircle, PaletteColor::TextMuted, "progress"),
      Self::Problem => (Icon::CircleAlert, PaletteColor::Warning, "problem"),
      Self::Note(glyph) => (glyph, PaletteColor::TextMuted, "note"),
    }
  }
}

impl Need {
  /// The need as what the person does: *add a title*.
  const fn verb_key(self) -> &'static str {
    match self {
      Self::Title => "task_form.reasons.need.title",
      Self::Brief => "task_form.reasons.need.brief",
      Self::Criteria => "task_form.reasons.need.criteria",
      Self::Pipeline => "task_form.reasons.need.pipeline",
      Self::NoPipelines => "task_form.reasons.need.noPipelines",
      Self::Budget => "task_form.reasons.need.budget",
      Self::TimeLimit => "task_form.reasons.need.timeLimit",
      Self::KeepComplete => "task_form.reasons.need.keepComplete",
      Self::NewerVersion => "task_form.reasons.need.newerVersion",
    }
  }

  /// The need as what a run also needs: *the outcome*.
  const fn noun_key(self) -> &'static str {
    match self {
      Self::Title => "task_form.reasons.also.title",
      Self::Brief => "task_form.reasons.also.brief",
      Self::Criteria => "task_form.reasons.also.criteria",
      Self::Pipeline => "task_form.reasons.also.pipeline",
      Self::NoPipelines => "task_form.reasons.also.noPipelines",
      Self::Budget => "task_form.reasons.also.budget",
      Self::TimeLimit => "task_form.reasons.also.timeLimit",
      Self::KeepComplete => "task_form.reasons.also.keepComplete",
      Self::NewerVersion => "task_form.reasons.also.newerVersion",
    }
  }
}

/// The line for `view`; `None` while there is nothing to say.
pub fn reason(ctx: &Ctx, view: &View) -> Option<Reason> {
  let say = |kind, words: Arc<str>| Some(Reason { kind, words });
  if let Some(failure) = view.failure.as_ref().and_then(|failures| failures.first()) {
    return say(Kind::Problem, failure_text(ctx, failure));
  }
  match &view.phase {
    Phase::Creating => return say(Kind::Busy, ctx.t("task_form.progress.creatingForRun")),
    Phase::Starting(_) if view.retrying => return say(Kind::Busy, ctx.t("task_form.queued.retryNote")),
    Phase::Starting(_) => return say(Kind::Busy, ctx.t("task_form.progress.starting")),
    Phase::Saving if view.subject.is_new() => return say(Kind::Busy, ctx.t("task_form.progress.creatingDraft")),
    Phase::Saving => return say(Kind::Busy, ctx.t("task_form.progress.saving")),
    Phase::NotStarted { problems, .. } => {
      return match problems.first() {
        Some(problem) => say(Kind::Problem, message_text(ctx, problem)),
        None => say(Kind::Note(Icon::RotateCcw), ctx.t("task_form.queued.retryNote")),
      };
    }
    Phase::Editing => {}
  }
  if let Some(conflict) = &view.conflict {
    let fields: Vec<Arc<str>> = conflict
      .unchosen(&view.choices)
      .into_iter()
      .map(|field| ctx.t(field_key(field)))
      .collect();
    let what = list(ctx, &fields)?;
    return say(
      Kind::Blocked,
      ctx.t_args("task_form.conflict.unchosen", [("fields", what.as_ref())]),
    );
  }
  editing(ctx, view)
}

/// While the person edits: what the next action needs, or what pressing it does.
fn editing(ctx: &Ctx, view: &View) -> Option<Reason> {
  let blocked = |words| {
    Some(Reason {
      kind: Kind::Blocked,
      words,
    })
  };
  match &view.subject {
    Subject::New => {
      let create = view.create_needs();
      if !create.is_empty() {
        let more: Vec<Need> = view
          .run_needs()
          .into_iter()
          .filter(|need| !create.contains(need))
          .collect();
        return blocked(create_and_run(ctx, &create, &more));
      }
    }
    Subject::Draft(_) => {
      if let Some(needs) = view.save_changes_needs().filter(|needs| !needs.is_empty()) {
        return blocked(needs_text(ctx, "task_form.reasons.save", &needs));
      }
    }
    Subject::Ran(_) => {
      return match view.save_changes_needs() {
        None => blocked(ctx.t("task_form.reasons.nothingChanged")),
        Some(needs) if !needs.is_empty() => blocked(needs_text(ctx, "task_form.reasons.save", &needs)),
        Some(_) => Some(Reason {
          kind: Kind::Note(Icon::HardDrive),
          words: ctx.t("task_form.reasons.keepsChanges"),
        }),
      };
    }
  }
  let run = view.run_needs();
  if !run.is_empty() {
    return blocked(needs_text(ctx, "task_form.reasons.run", &run));
  }
  Some(Reason {
    kind: Kind::Ready,
    words: ctx.t(match delivered(view) {
      Some(Deliverable::Report) => "task_form.reasons.readyReport",
      Some(Deliverable::Outside) => "task_form.reasons.readyOutside",
      Some(Deliverable::Changes | Deliverable::ChangesOrReport) | None => "task_form.reasons.ready",
    }),
  })
}

/// *To create, …*, and what a run needs beyond it: *To run, also …*.
fn create_and_run(ctx: &Ctx, create: &[Need], more: &[Need]) -> Arc<str> {
  let verbs: Vec<Arc<str>> = create.iter().map(|need| ctx.t(need.verb_key())).collect();
  let nouns: Vec<Arc<str>> = more.iter().map(|need| ctx.t(need.noun_key())).collect();
  let what = list(ctx, &verbs).unwrap_or_default();
  match list(ctx, &nouns) {
    Some(more) => ctx.t_args(
      "task_form.reasons.createAndRun",
      [("what", what.as_ref()), ("more", more.as_ref())],
    ),
    None => ctx.t_args("task_form.reasons.create", [("what", what.as_ref())]),
  }
}

/// `key` with what `needs` ask the person to do.
fn needs_text(ctx: &Ctx, key: &str, needs: &[Need]) -> Arc<str> {
  let words: Vec<Arc<str>> = needs.iter().map(|need| ctx.t(need.verb_key())).collect();
  let what = list(ctx, &words).unwrap_or_default();
  ctx.t_args(key, [("what", what.as_ref())])
}

/// `items` as a list in words: "a", "a and b", "a, b and c".
fn list(ctx: &Ctx, items: &[Arc<str>]) -> Option<Arc<str>> {
  match items {
    [] => None,
    [one] => Some(one.clone()),
    [first, second] => Some(ctx.t_args(
      "task_form.reasons.list.two",
      [("first", first.as_ref()), ("second", second.as_ref())],
    )),
    [first, rest @ ..] => {
      let rest = list(ctx, rest)?;
      Some(ctx.t_args(
        "task_form.reasons.list.more",
        [("first", first.as_ref()), ("rest", rest.as_ref())],
      ))
    }
  }
}

/// What a failure says: a change met meanwhile in the form's own words (the form reads the newer
/// version itself), anything else in the workspace's.
fn failure_text(ctx: &Ctx, failure: &orchester_workspace::messages::Message) -> Arc<str> {
  if failure.key == orchester_workspace::messages::task_changed_meanwhile().key {
    return ctx.t("task_form.conflict.changedMeanwhile");
  }
  message_text(ctx, failure)
}
