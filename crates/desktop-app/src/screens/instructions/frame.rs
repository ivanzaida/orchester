//! The Instructions tab with no instruction yet (06b, `B5A4Ux`), with what the repository already gives agents.
use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::instructions::data::ProjectInstructions;

use super::state::{Selection, Tab};
use crate::project::ShownProject;

pub fn empty(
  ctx: &mut Ctx,
  project: &ShownProject,
  (data, tab): (&ProjectInstructions, Tab),
  selection: &Selection,
) -> Element {
  ctx.mount_keyed::<super::empty::Empty>(
    "instructions-empty",
    super::empty::Props {
      project: project.clone(),
      selection: selection.clone(),
      all_empty: data.entries.is_empty(),
      tab,
    },
  )
}
