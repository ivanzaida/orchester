//! The panel's body (`Steps` `uSoll` in `Pipelines · v2` P1): the section's head over the steps drawn as the flow they
//! run in ([`super::flow`]), drawn from the edit the panel holds ([`super::edit`]) while there is one. *Ready?* is
//! PV2-09's.
//!
//! The head's *Add step* adds a step at the end of the line, and a card changes its place, is duplicated or removed
//! through the same edit (PV2-06), unless the panel is locked while a save runs or its question is open; where the
//! steps have no order to draw the flow's notice offers *Make it a line* (P16c `YHAyx`).

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::PipelineEdit;
use orchester_workspace::pipelines::flow::FlowRead;

use super::{edit::Edit, flow, model::Entry};
use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, HubText},
};

/// What the body shows of `entry`: the steps section with the flow `read`, which is the edit's while the panel holds
/// one, and the moves made on `current`; nothing for an archived pipeline, which has no steps to open.
pub(super) fn parts(
  ctx: &mut Ctx,
  project: &ShownProject,
  entry: &Entry,
  (read, edit, current): (&Arc<FlowRead>, &Edit, &PipelineEdit),
  (locked, pick_agent): (bool, &Signal<bool>),
) -> Vec<Element> {
  if entry.archived.is_some() {
    return Vec::new();
  }
  vec![flow::section(
    ctx,
    (project, entry.id, entry.step_count),
    read,
    (edit, current),
    (locked, pick_agent),
  )]
}

/// *Steps 6* and, at the end, `action`: *Add step*, none where the steps cannot be drawn.
pub(super) fn head(ctx: &Ctx, count: usize, action: Option<Element>) -> Element {
  Row::new()
    .id("pipeline-hub-steps")
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t("pipeline_hub.panel.steps"))
            .id("pipeline-hub-steps-label")
            .variant(HubText::PanelLabel.role()),
        )
        .child(
          Text::new(&count.to_string())
            .id("pipeline-hub-steps-count")
            .variant(HubText::PanelHint.role()),
        ),
    )
    .with_children(action)
    .into()
}
