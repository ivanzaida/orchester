//! The panel's body (`Steps` `uSoll` in `Pipelines · v2` P1): the section's head over the steps drawn as the flow they
//! run in ([`super::flow`]). The pipeline's settings and *Ready?* (PV2-09) and the Save bar (PV2-10) are built on this.
//!
//! The head's *Add step* needs the page to keep an unsaved edit (PV2-10), and a card's drag handle and ⋯ are PV2-06's,
//! so none is drawn yet: *Edit steps* opens the editor the pipeline has today, through its own route, where steps are
//! added, moved and removed until then.

use lurq::{
  app::ctx::Ctx,
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{flow, model::Entry};
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

/// What the body shows of `entry`: the head and the flow; nothing for an archived pipeline, which has no steps to open.
pub(super) fn parts(ctx: &mut Ctx, project: &ShownProject, entry: &Entry) -> Vec<Element> {
  let (Some(read), None) = (&entry.flow, entry.archived) else {
    return Vec::new();
  };
  let navigation = Navigation::of(ctx);
  let count = read.flow.as_ref().map_or(entry.step_count, |flow| flow.len());
  vec![
    head(ctx, project, &navigation, (entry, count)),
    flow::flow(ctx, (project, &navigation), entry.id, &read.flow),
  ]
}

/// *Steps 6* and, at the end, *Edit steps*.
fn head(ctx: &Ctx, project: &ShownProject, navigation: &Navigation, (entry, count): (&Entry, usize)) -> Element {
  let route = Route::Project(
    project.open.project.id(),
    ProjectPage::Pipeline(entry.id, PipelineView::default()),
  );
  let edit = ActionButton::new(
    "pipeline-hub-edit-steps",
    ButtonKind::Ghost,
    ctx.t("pipeline_hub.panel.openSteps"),
  )
  .icon(Icon::Pencil)
  .opens(navigation.to(route));
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
    .child(edit)
    .into()
}
