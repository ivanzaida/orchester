//! The Deliver step opened in the pipeline's panel (`Pipelines · v2` P15 `nPaMj`), as a step opens (P2): the head with
//! ← back to the pipeline, *step 06 of 6* over *Deliver* and *Remove step*; the strip with Deliver as the open step;
//! then its fields in groups: *Mode* ([`mode`]), *Merge* ([`merge`]), *Conflicts* ([`conflicts`]) and *Check*
//! ([`check`]).
//!
//! Every field changes the Deliver step of the edit the panel holds and nothing else; *Save* and *Cancel* are the
//! panel's and act on the whole pipeline. *Remove step* takes the step off the edit without a question and shows the
//! pipeline, where *Add Deliver step* stands in its place (P15b `uDfc0`); *Cancel* puts it back.

mod check;
mod conflicts;
mod merge;
mod mode;

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{DeliverStep, PipelineEdit};
use orchester_workspace::pipelines::{data::Assignables, flow::FlowRead};

use super::{head::where_line, pieces, strip};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{PipelinesFilter, PipelinesView, ProjectPage, Route},
  screens::pipeline_hub::{OpenStep, edit::Edit, model::Entry},
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// What the panel gives the open Deliver step.
pub(in crate::screens::pipeline_hub) struct Open<'a> {
  pub project: &'a ShownProject,
  pub entry: &'a Entry,
  pub edit: &'a Edit,
  /// What the panel shows: the edit, or the stored pipeline.
  pub current: &'a PipelineEdit,
  pub read: &'a Arc<FlowRead>,
  pub assignables: &'a Assignables,
  /// A save is under way, or a question about it waits: nothing is changed now.
  pub locked: bool,
  /// How many times the panel put the edit back or stored it: the fields are mounted anew after each.
  pub epoch: u64,
}

/// What the panel's body shows for the Deliver step: the head, the strip and the groups; nothing when the pipeline has
/// no Deliver step.
pub(in crate::screens::pipeline_hub) fn parts(ctx: &mut Ctx, open: &Open<'_>) -> Vec<Element> {
  let Some(deliver) = open.current.deliver() else {
    return Vec::new();
  };
  let pieces = pieces::of(open.read, open.current, &ctx.t("pipeline_hub.flow.deliver"));
  let place = pieces::place(&pieces, None).unwrap_or((0, 0));
  vec![
    head(ctx, open, place),
    strip::strip(
      ctx,
      strip::Props {
        project: open.project.open.project.id(),
        pipeline: open.entry.id,
        pieces,
        current: OpenStep::Deliver,
      },
    ),
    mode::group(ctx, open, deliver),
    merge::group(ctx, open, deliver),
    conflicts::group(ctx, open, deliver),
    check::group(ctx, open, deliver),
  ]
}

/// Changes the edit's Deliver step with `change`, unless it has none.
fn change(edit: &Edit, change: impl FnOnce(DeliverStep) -> DeliverStep) {
  edit.change(|current| {
    if let Some(deliver) = current.deliver().cloned() {
      current.set_deliver(Some(change(deliver)));
    }
  });
}

/// The head (`Head` `IOh0z`): the line back to the pipeline over *Deliver*, and *Remove step* at its end.
fn head(ctx: &mut Ctx, open: &Open<'_>, place: (usize, usize)) -> Element {
  let project = open.project.open.project.id();
  let where_ = where_line(ctx, (project, open.entry), place);
  Row::new()
    .id("pipeline-hub-deliver-head")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Xs)
        .child(where_)
        .child(
          Text::new(&ctx.t("pipeline_hub.deliver.title"))
            .id("pipeline-hub-deliver-title")
            .variant(HubText::PanelTitle.role())
            .nowrap(),
        ),
    )
    .child(remove(ctx, open))
    .into()
}

/// *Remove step*: the edit ends in no Deliver step, and the pipeline shows; nothing is stored until *Save*.
fn remove(ctx: &mut Ctx, open: &Open<'_>) -> Element {
  let button = ActionButton::new(
    "pipeline-hub-deliver-remove",
    ButtonKind::DangerGhost,
    ctx.t("pipeline_hub.deliver.remove"),
  )
  .icon(Icon::Trash2);
  if open.locked {
    return button.disabled().into();
  }
  let (edit, navigation) = (open.edit.clone(), Navigation::of(ctx));
  let pipeline = Route::Project(
    open.project.open.project.id(),
    ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(open.entry.id))),
  );
  button
    .on_click(move || {
      edit.change(|current| current.set_deliver(None));
      navigation.go(pipeline.clone());
    })
    .into()
}
