//! The panel's body (`Steps` `uSoll` in `Pipelines · v2` P1). The steps drawn as the flow they run in (PV2-05), the
//! pipeline's settings and *Ready?* (PV2-09) and the Save bar (PV2-10) are built on this; until they land the body
//! offers the editor the pipeline has today, through its own route, so that its steps can still be changed.

use lurq::{
  app::ctx::Ctx,
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::model::Entry;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{PipelineView, ProjectPage, Route},
  theme::{ExtraSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// What the body shows of `entry`: *Steps* with their count and *Edit steps*, which opens the pipeline's page; nothing
/// for an archived pipeline, which has no page to open.
pub(super) fn parts(ctx: &mut Ctx, project: &ShownProject, entry: &Entry) -> Vec<Element> {
  if entry.archived.is_some() {
    return Vec::new();
  }
  let route = Route::Project(
    project.open.project.id(),
    ProjectPage::Pipeline(entry.id, PipelineView::default()),
  );
  let open = ActionButton::new(
    "pipeline-hub-edit-steps",
    ButtonKind::Secondary,
    ctx.t("pipeline_hub.panel.openSteps"),
  )
  .icon(Icon::Pencil)
  .opens(Navigation::of(ctx).to(route));
  vec![
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
      .child(open)
      .into(),
  ]
}
