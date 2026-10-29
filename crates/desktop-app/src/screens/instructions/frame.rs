//! The Instructions tab with no instruction yet (06b `VcE5a`), with what the repository already gives agents.
use lurq::{app::ctx::Ctx, node::Element};

use super::state::Selection;
use crate::project::ShownProject;

pub fn empty(ctx: &mut Ctx, project: &ShownProject, selection: &Selection) -> Element {
  ctx.mount_keyed::<super::empty::Empty>(
    "instructions-empty",
    super::empty::Props {
      project: project.clone(),
      selection: selection.clone(),
    },
  )
}
