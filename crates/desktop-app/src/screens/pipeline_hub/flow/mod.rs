//! The steps of a pipeline drawn as the flow they run in (`Flow` `CX0tV` in `Steps` `uSoll`, `Pipelines · v2` P1; the
//! amber card of P8): a vertical line with a node per step from *ON TASK START* to where the run ends, a card per step,
//! an approval gate on the line after its step, steps that run side by side in one *RUN IN PARALLEL* frame, and where a
//! failure sends the run back on its card. Nothing of the layout is stored: it is read from the steps' dependencies
//! (`orchester_workspace::pipelines::flow`). A pipeline with no steps, or whose steps one line cannot show, keeps the
//! start and the end and says so between them. A card opens its step through today's step page until the step opens in
//! the panel (PV2-07).
//!
//! The order is changed here, through the edit the panel holds ([`super::edit`]): a card the pointer or the keys are on
//! shows its drag handle and its ⋯ menu (P3 `K4B56`, P4 `naN3G`); *Add step* in the section's head adds one. Nothing
//! is saved until *Save*.

mod card;
mod drag;
mod fix;
mod landing;
mod menu;
mod parts;
mod rail;
mod reorder;
mod state;
mod view;
mod words;

use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Button, node::Element};
use orchester_core::{
  id::ResourceId,
  resource::{FlowError, PipelineEdit},
};
use orchester_workspace::pipelines::flow::FlowRead;

pub(super) use self::{fix::Fix, words::number};
use super::edit::Edit;
use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{PipelineView, ProjectPage, Route},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
    pressable::Press,
  },
};

/// How the flow reaches the routes of the steps.
struct Opens<'a> {
  navigation: &'a Navigation,
  project: &'a ShownProject,
  pipeline: ResourceId,
}

impl Opens<'_> {
  fn open(&self, view: PipelineView) -> Open {
    self.navigation.to(Route::Project(
      self.project.open.project.id(),
      ProjectPage::Pipeline(self.pipeline, view),
    ))
  }

  fn button(&self, view: PipelineView) -> Button {
    Press::Open(self.open(view)).button()
  }

  fn fix(&self, fix: Fix) -> Button {
    let route = Route::Project(self.project.open.project.id(), fix.page());
    Press::Open(self.navigation.to(route)).button()
  }
}

/// *Edit steps*: the editor the pipeline has today, where steps are added, moved and removed until the panel does.
pub(super) fn edit_steps(ctx: &Ctx, id: &str, open: Open) -> ActionButton {
  ActionButton::new(id.to_owned(), ButtonKind::Ghost, ctx.t("pipeline_hub.panel.openSteps"))
    .icon(Icon::Pencil)
    .opens(open)
}

/// The steps section of pipeline `pipeline`: its head with *Add step*, and the flow from the start to the end, with the
/// steps, or the note that says why there are none to draw, between them. `read` is what is drawn, `current` the edit
/// the moves are made on.
pub(in crate::screens::pipeline_hub) fn section(
  ctx: &mut Ctx,
  (project, pipeline, count): (&ShownProject, ResourceId, usize),
  read: &Arc<FlowRead>,
  (edit, current): (&Edit, &PipelineEdit),
) -> Element {
  ctx.mount_keyed::<view::FlowView>(
    &format!("pipeline-hub-flow-{pipeline}"),
    view::Props {
      project: project.clone(),
      pipeline,
      count,
      read: read.clone(),
      current: current.clone(),
      edit: edit.clone(),
    },
  )
}

/// Steps one line cannot show: a step that waits for only some of what runs before it, or steps that wait for each
/// other. They are not guessed at; the page that edits steps, which the notice opens, shows their dependencies.
fn undrawable(ctx: &Ctx, opens: &Opens<'_>, error: &FlowError) -> Element {
  let (title, body) = match error {
    FlowError::Branches(_) => ("pipeline_hub.flow.branches.title", "pipeline_hub.flow.branches.detail"),
    FlowError::Broken(_) => ("pipeline_hub.flow.broken.title", "pipeline_hub.flow.broken.detail"),
  };
  Notice::new("pipeline-hub-flow-undrawable", NoticeTone::Warning, ctx.t(title))
    .flat()
    .body(ctx.t(body))
    .action(edit_steps(
      ctx,
      "pipeline-hub-flow-undrawable-edit",
      opens.open(PipelineView::default()),
    ))
    .into()
}
