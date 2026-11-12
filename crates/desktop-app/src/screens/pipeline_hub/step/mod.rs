//! A step opened in the pipeline's panel (`Pipelines · v2` P2 `u2rYEH`, P2b `EGEos`, P13 `KRQ5J`): the head with ← back
//! to the pipeline and the step's name edited in place ([`head`], [`name`]), the strip of the whole flow that switches
//! steps ([`strip`]) and the step's fields in groups: *Step* (*Agent* with *Open Builder*, and *Instructions*) and
//! *Run* (what happens when changes are requested, the step budget, *When the step fails*) ([`groups`]), and *Gate and
//! access* with the step's permissions ([`gate`], [`permissions`]). Where a step runs is set in the flow; *Run* has
//! *Waits for* only for a step that waits for part of the stage before it, or whose row the person opened (P16b
//! `P1853`, [`waits`]).
//!
//! Every field changes the edit the panel holds ([`super::edit`]) and nothing else; *Save* and *Cancel* are the
//! panel's and act on the whole pipeline. The route holds the open step (`ProjectPage::Pipeline` with
//! `PipelineView::Step`), so Back and Forward move between the pipeline and the step. A step's fields are mounted
//! anew when another step opens and after each save or *Cancel*, so a box never keeps what the edit no longer has.

mod agent;
mod choices;
mod executor;
mod gate;
mod groups;
mod head;
mod known;
mod more;
mod name;
mod permissions;
mod pieces;
mod strip;
mod text_field;
mod waits;

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::{ElementRef, Signal},
  node::Element,
};
use orchester_core::resource::{PipelineEdit, StepKey};
use orchester_workspace::pipelines::{data::Assignables, flow::FlowRead};

pub(super) use self::known::Paths;
use super::{edit::Edit, model::Entry};
use crate::project::ShownProject;

/// Text typed into a field of a step that is not valid, kept while the person goes to another step or back to the
/// pipeline, so that it is neither lost nor taken for what the edit holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Invalid {
  step: StepKey,
  kind: text_field::Kind,
  text: String,
}
impl DevtoolsInspectable for Invalid {}

/// The invalid texts of the pipeline's steps: while there is one, the panel does not offer to save and its bar says
/// which field to fix. *Cancel* and the question that discards the edit clear them.
pub(super) type Faulty = Signal<Vec<Invalid>>;

/// The invalid text of `kind` of `step` in `faulty`, if one is kept.
fn kept(faulty: &Faulty, step: &StepKey, kind: text_field::Kind) -> Option<String> {
  faulty
    .get_untracked()
    .iter()
    .find(|invalid| invalid.step == *step && invalid.kind == kind)
    .map(|invalid| invalid.text.clone())
}

/// Keeps `text`, which is not valid, as the text of `kind` of `step`, or (`None`) forgets it; written only when it
/// differs so the panel is not drawn for nothing.
fn keep(faulty: &Faulty, (step, kind): (&StepKey, text_field::Kind), text: Option<&str>) {
  if kept(faulty, step, kind).as_deref() == text {
    return;
  }
  faulty.update(|all| {
    all.retain(|invalid| !(invalid.step == *step && invalid.kind == kind));
    if let Some(text) = text {
      all.push(Invalid {
        step: step.clone(),
        kind,
        text: text.to_owned(),
      });
    }
  });
}

/// The invalid texts of boxes the pipeline still shows: those of steps it still has, and the retries only of a step
/// that still returns to an earlier one on failure.
fn live<'a>(all: &'a [Invalid], edit: &'a PipelineEdit) -> impl Iterator<Item = &'a Invalid> {
  all.iter().filter(|invalid| {
    edit
      .step(&invalid.step)
      .is_some_and(|step| invalid.kind != text_field::Kind::Retries || step.on_failure.is_some())
  })
}

/// Forgets the invalid text of steps `edit` no longer has. A step removed and a copy made later can take the same key,
/// and the removed one's text would stand in the new one's box.
pub(super) fn forget_gone(faulty: &Faulty, edit: &PipelineEdit) {
  if faulty
    .get_untracked()
    .iter()
    .all(|invalid| edit.step(&invalid.step).is_some())
  {
    return;
  }
  faulty.update(|all| all.retain(|invalid| edit.step(&invalid.step).is_some()));
}

/// Whether a step of `edit` holds text that is not valid, which Save must not store around.
pub(super) fn holds_invalid(faulty: &Faulty, edit: &PipelineEdit) -> bool {
  live(&faulty.get(), edit).next().is_some()
}

/// What the bar says while Save is not offered: which field of which step to fix; `None` when nothing is wrong.
pub(super) fn fix_words(ctx: &Ctx, faulty: &Faulty, edit: &PipelineEdit) -> Option<String> {
  let all = faulty.get();
  let invalid = live(&all, edit).next()?;
  let step = edit.step(&invalid.step)?;
  Some(
    ctx
      .t_args(
        "pipeline_hub.bar.fix",
        [
          ("field", ctx.t(invalid.kind.label()).to_string()),
          ("step", step.name.as_str().to_owned()),
        ],
      )
      .to_string(),
  )
}

/// What the panel gives the open step.
pub(super) struct Open<'a> {
  pub project: &'a ShownProject,
  pub entry: &'a Entry,
  pub key: &'a StepKey,
  pub edit: &'a Edit,
  /// What the panel shows: the edit, or the stored pipeline.
  pub current: &'a PipelineEdit,
  pub read: &'a Arc<FlowRead>,
  pub assignables: &'a Assignables,
  /// A save is under way: nothing is changed now.
  pub locked: bool,
  /// How many times the panel put the edit back or stored it: the fields are mounted anew after each.
  pub epoch: u64,
  pub faulty: &'a Faulty,
  /// Set by the Deliver chip, which opens the pipeline and has its *A run delivers* row shown.
  pub reveal_delivers: &'a Signal<bool>,
  /// Set by a *Choose another* that opened this step: its Agent field opens its picker once.
  pub pick_agent: &'a Signal<bool>,
  /// The last known CLI each step of the edit ran on ([`known`]).
  pub known: &'a known::PathsSignal,
  /// *Gate and access*, which the panel brings into view with the question before a save.
  pub gate: &'a ElementRef,
}

/// What the panel's body shows for the open step: the head, the strip and the groups of fields; nothing when the
/// pipeline has no such step.
pub(super) fn parts(ctx: &mut Ctx, open: &Open<'_>) -> Vec<Element> {
  let Some(step) = open.current.step(open.key) else {
    return Vec::new();
  };
  let pieces = pieces::of(open.read, open.current, &ctx.t("pipeline_hub.flow.deliver"));
  let place = pieces::place(&pieces, open.key).unwrap_or((0, 0));
  let upstream = pieces::upstream(&pieces, open.key);
  vec![
    head::head(ctx, open, step, place),
    strip::strip(
      ctx,
      strip::Props {
        project: open.project.open.project.id(),
        pipeline: open.entry.id,
        pieces,
        current: open.key.clone(),
        reveal_delivers: open.reveal_delivers.clone(),
      },
    ),
    groups::step(ctx, open, step),
    groups::run(ctx, open, step, &upstream),
    gate::group(ctx, open, step),
  ]
}
