//! The panel's body (`Steps` `uSoll` in `Pipelines · v2` P1): the section's head over the steps drawn as the flow they
//! run in ([`super::flow`]), drawn from the edit the panel holds ([`super::edit`]) while there is one. *Ready?* is
//! PV2-09's.
//!
//! The page keeps the edit now (PV2-10), but the head's *Add step* is PV2-07's and a card's drag handle and ⋯ are
//! PV2-06's, so none is drawn yet: *Edit steps* opens the editor the pipeline has today, through its own route (the
//! leave guard asks first while the edit has changes), where steps are added, moved and removed until then.

use lurq::{
  app::ctx::Ctx,
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::pipelines::flow::FlowRead;

use super::{flow, model::Entry};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{PipelineView, ProjectPage, Route},
  theme::{ExtraSpacing, HubText},
};

/// What the body shows of `entry`: the head and the flow `read`, which is the edit's while the panel holds one;
/// nothing for an archived pipeline, which has no steps to open.
pub(super) fn parts(ctx: &mut Ctx, project: &ShownProject, entry: &Entry, read: &FlowRead) -> Vec<Element> {
  if entry.archived.is_some() {
    return Vec::new();
  }
  let navigation = Navigation::of(ctx);
  vec![
    head(ctx, project, &navigation, entry),
    flow::flow(ctx, (project, &navigation), entry.id, read),
  ]
}

/// *Steps 6* and, at the end, *Edit steps*.
fn head(ctx: &Ctx, project: &ShownProject, navigation: &Navigation, entry: &Entry) -> Element {
  let route = Route::Project(
    project.open.project.id(),
    ProjectPage::Pipeline(entry.id, PipelineView::default()),
  );
  let edit = flow::edit_steps(ctx, "pipeline-hub-edit-steps", navigation.to(route));
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
          Text::new(&entry.step_count.to_string())
            .id("pipeline-hub-steps-count")
            .variant(HubText::PanelHint.role()),
        ),
    )
    .child(edit)
    .into()
}
