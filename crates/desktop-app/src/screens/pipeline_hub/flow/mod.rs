//! The steps of a pipeline drawn as the flow they run in (`Flow` `CX0tV` in `Steps` `uSoll`, `Pipelines · v2` P1; the
//! amber card of P8): a vertical line with a node per step from *ON TASK START* to where the run ends, a card per step,
//! an approval gate on the line after its step, steps that run side by side in one *RUN IN PARALLEL* frame, and where a
//! failure sends the run back on its card. Nothing of the layout is stored: it is read from the steps' dependencies
//! (`orchester_workspace::pipelines::flow`). A pipeline with no steps, or whose steps one line cannot show, keeps the
//! start and the end and says so between them. A card opens its step through today's step page until the step opens in
//! the panel (PV2-07).
//!
//! The order is changed here, through the edit the panel holds ([`super::edit`]): a card the pointer or the keys are on
//! shows its drag handle and its ⋯ menu (P3 `K4B56`, P4 `naN3G`); *Add step* in the section's head adds one, and
//! *Add Deliver step* where the Deliver card would be adds that ([`add_deliver`], P15b `uDfc0`). Nothing is saved until
//! *Save*.

mod add_deliver;
mod card;
mod drag;
mod fix;
mod landing;
mod menu;
mod parts;
mod rail;
mod read_only;
mod reorder;
mod state;
mod step_menu;
mod view;
mod words;

use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Button, core::Signal, node::Element};
use orchester_core::{
  id::ResourceId,
  resource::{FlowError, PipelineEdit},
};
use orchester_workspace::pipelines::flow::FlowRead;

pub(super) use self::{
  fix::Fix,
  menu::OpensMade,
  reorder::{copy_name, copy_of},
  step_menu::{StepMenu, Target},
  words::number,
};
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
  /// The flow only shows the steps of a published version: nothing in it opens a step or the editor.
  read_only: bool,
  /// Set by a card whose profile or tool is gone: the step it opens opens its Agent picker.
  pick_agent: Option<Signal<bool>>,
}

impl Opens<'_> {
  fn open(&self, view: PipelineView) -> Open {
    self.navigation.to(Route::Project(
      self.project.open.project.id(),
      ProjectPage::Pipeline(self.pipeline, view),
    ))
  }

  fn button(&self, view: PipelineView) -> Option<Button> {
    (!self.read_only).then(|| Press::Open(self.open(view)).button())
  }

  fn fix(&self, fix: Fix) -> Option<Button> {
    let route = Route::Project(self.project.open.project.id(), fix.page());
    (!self.read_only).then(|| Press::Open(self.navigation.to(route)).button())
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
  (locked, pick_agent): (bool, &Signal<bool>),
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
      locked,
      pick_agent: pick_agent.clone(),
    },
  )
}

/// [`section`]'s flow of a published version viewed read-only: the same line, cards and gates, but no card opens a
/// step, shows the `›`, a handle or a ⋯, or takes the hover or the keys, there is no *Add step* or *Edit steps*, and
/// nothing in it moves.
pub(super) fn read_only(
  ctx: &Ctx,
  (project, navigation): (&ShownProject, &Navigation),
  pipeline: ResourceId,
  read: &FlowRead,
) -> Element {
  let opens = Opens {
    navigation,
    project,
    pipeline,
    read_only: true,
    pick_agent: None,
  };
  read_only::drawn(ctx, &opens, read)
}

/// Steps one line cannot show: a step that waits for only some of what runs before it, or steps that wait for each
/// other. They are not guessed at; the page that edits steps, which the notice opens, shows their dependencies.
fn undrawable(ctx: &Ctx, opens: &Opens<'_>, error: &FlowError) -> Element {
  let (title, body) = match error {
    FlowError::Branches(_) => ("pipeline_hub.flow.branches.title", "pipeline_hub.flow.branches.detail"),
    FlowError::Broken(_) => ("pipeline_hub.flow.broken.title", "pipeline_hub.flow.broken.detail"),
  };
  let notice = Notice::new("pipeline-hub-flow-undrawable", NoticeTone::Warning, ctx.t(title))
    .flat()
    .body(ctx.t(body));
  if opens.read_only {
    return notice.into();
  }
  notice
    .action(edit_steps(
      ctx,
      "pipeline-hub-flow-undrawable-edit",
      opens.open(PipelineView::default()),
    ))
    .into()
}
