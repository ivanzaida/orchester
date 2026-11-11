//! The head of an open step (`Head` in `Pipelines · v2` P2 `u2rYEH`): ← and the pipeline's name, which return to the
//! pipeline's panel, and *step 03 of 6* over the step's name, with *Duplicate* at the end. The step's own ⋯ menu is the
//! flow card's (P4 `naN3G`): it moves and removes steps, which the flow owns.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::Step;

use super::{Open, name};
use crate::{
  navigation::Navigation,
  routes::{PipelineView, PipelinesFilter, PipelinesView, ProjectPage, Route},
  screens::pipeline_hub::flow::{copy_name, copy_of, number},
  theme::{ExtraSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::{Focus, Press},
  },
};

/// The head of `step`, which stands at `place`: its number and how many steps the pipeline has.
pub(super) fn head(ctx: &mut Ctx, open: &Open<'_>, step: &Step, place: (usize, usize)) -> Element {
  let navigation = Navigation::of(ctx);
  let project = open.project.open.project.id();
  let back = Route::Project(
    project,
    ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(open.entry.id))),
  );
  let pipeline = open.entry.name.clone();
  let back = Focus::Keyboard
    .apply(Press::Open(navigation.to(back)).button())
    .id("pipeline-hub-step-back")
    .describe(
      "a11y_name",
      ctx.t_args("pipeline_hub.step.back", [("name", &*pipeline)]),
    )
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(
      Icon::ArrowLeft,
      IconSize::Sm,
      lurq::app::theme::PaletteColor::TextSecondary,
    ))
    .child(
      Text::new(&pipeline)
        .id("pipeline-hub-step-back-name")
        .variant(HubText::PanelWhere.role())
        .nowrap(),
    );
  let where_ = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(back)
    .child(
      Text::new(&ctx.t_args(
        "pipeline_hub.step.position",
        [("number", number(place.0)), ("total", place.1.to_string())],
      ))
      .id("pipeline-hub-step-position")
      .variant(HubText::PanelHint.role())
      .nowrap(),
    );
  // The name's box stands 8 to the start of the name (D-022), past the head's start: neither the head nor its column
  // cuts what stands past them, so its ring and hover fill draw whole.
  Row::new()
    .id("pipeline-hub-step-head")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .overflow_visible()
    .child(
      // The name's box is 28 high with the name in its middle, which brings the name as near the line over it as the
      // other spacing would.
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .overflow_visible()
        .child(where_)
        .child(name::field(
          ctx,
          name::Props {
            edit: open.edit.clone(),
            key: step.key.clone(),
            shown: step.name.as_str().to_owned(),
            label: ctx.t("pipeline_hub.stepEdit.name"),
            locked: open.locked,
            epoch: open.epoch,
            faulty: open.faulty.clone(),
          },
        )),
    )
    .child(duplicate(ctx, open, step, &navigation))
    .into()
}

/// *Duplicate*: a copy of the step right after it, opened at once; the edit holds it until *Save*.
fn duplicate(ctx: &Ctx, open: &Open<'_>, step: &Step, navigation: &Navigation) -> Element {
  let name = copy_name(ctx, step.name.as_str());
  let (edit, key, navigation) = (open.edit.clone(), step.key.clone(), navigation.clone());
  let (project, pipeline) = (open.project.open.project.id(), open.entry.id);
  let button = ActionButton::new(
    "pipeline-hub-step-duplicate",
    ButtonKind::Ghost,
    ctx.t("pipeline_hub.step.duplicate"),
  )
  .icon(Icon::Copy);
  if open.locked {
    return button.disabled().into();
  }
  button
    .on_click(move || {
      let mut copy = None;
      edit.change(|current| copy = copy_of(current, &key, name.clone()));
      if let Some(copy) = copy {
        navigation.go(Route::Project(
          project,
          ProjectPage::Pipeline(pipeline, PipelineView::Step(copy)),
        ));
      }
    })
    .into()
}
