//! A step opened in the pipeline's panel (`Pipelines · v2` P2 `u2rYEH`, P13 `KRQ5J`): the head with ← back to the
//! pipeline ([`head`]), the strip of the whole flow that switches steps ([`strip`]) and the step's fields in groups:
//! *Step* (*Agent* and *Instructions*), *Run* (what happens when changes are requested, the step budget) and *Gate and
//! access* ([`groups`]). There is no *Runs after*: where a step runs is set in the flow.
//!
//! Every field changes the edit the panel holds ([`super::edit`]) and nothing else; *Save* and *Cancel* are the
//! panel's and act on the whole pipeline. The route holds the open step (`ProjectPage::Pipeline` with
//! `PipelineView::Step`), so Back and Forward move between the pipeline and the step. A step's fields are mounted
//! anew when another step opens and after each save or *Cancel*, so a box never keeps what the edit no longer has.

mod agent;
mod groups;
mod head;
mod pieces;
mod strip;
mod text_field;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::resource::{PipelineEdit, StepKey};
use orchester_workspace::pipelines::{data::Assignables, flow::FlowRead};

use super::{edit::Edit, model::Entry};
use crate::project::ShownProject;

/// The fields of the open step that show an error, by name: while one does, what is typed is not what the edit holds,
/// so the panel does not offer to save.
pub(super) type Faulty = Signal<Vec<&'static str>>;

/// Marks or clears the error of field `field`; written only when it differs so the panel is not drawn for nothing.
fn mark(faulty: &Faulty, field: &'static str, bad: bool) {
  let marked = faulty.get_untracked().contains(&field);
  if marked == bad {
    return;
  }
  faulty.update(|fields| {
    if bad {
      fields.push(field);
    } else {
      fields.retain(|name| *name != field);
    }
  });
}

/// What the panel gives the open step.
pub(super) struct Open<'a> {
  pub project: &'a ShownProject,
  pub entry: &'a Entry,
  pub key: &'a StepKey,
  pub edit: &'a Edit,
  /// What the panel shows: the edit, or the stored pipeline.
  pub current: &'a PipelineEdit,
  pub read: &'a FlowRead,
  pub assignables: &'a Assignables,
  /// A save is under way: nothing is changed now.
  pub locked: bool,
  /// How many times the panel put the edit back or stored it: the fields are mounted anew after each.
  pub epoch: u64,
  pub faulty: &'a Faulty,
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
      },
    ),
    groups::step(ctx, open, step),
    groups::run(ctx, open, step, &upstream),
    groups::gate(ctx, open, step),
  ]
}
