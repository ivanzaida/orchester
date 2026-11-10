//! The head of an open step (`Head` in `Pipelines · v2` P2 `u2rYEH`): ← and the pipeline's name, which return to the
//! pipeline's panel, and *step 03 of 6* over the step's name, with *Duplicate* at the end. The step's own ⋯ menu is the
//! flow card's (P4 `naN3G`): it moves and removes steps, which the flow owns.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  name::Name,
  resource::{PipelineEdit, Step, StepKey},
};

use super::Open;
use crate::{
  navigation::Navigation,
  routes::{PipelineView, PipelinesFilter, PipelinesView, ProjectPage, Route},
  screens::pipeline_hub::flow::number,
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
  Row::new()
    .id("pipeline-hub-step-head")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Xs)
        .child(where_)
        .child(
          Text::new(step.name.as_str())
            .id("pipeline-hub-step-title")
            .variant(HubText::PanelTitle.role()),
        ),
    )
    .child(duplicate(ctx, open, step, &navigation))
    .into()
}

/// *Duplicate*: a copy of the step right after it, opened at once; the edit holds it until *Save*.
fn duplicate(ctx: &Ctx, open: &Open<'_>, step: &Step, navigation: &Navigation) -> Element {
  let name = ctx.t_args("pipeline_hub.step.copyName", [("name", step.name.as_str())]);
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
      edit.change(|current| copy = copy_of(current, &key, &name));
      if let Some(copy) = copy {
        navigation.go(Route::Project(
          project,
          ProjectPage::Pipeline(pipeline, PipelineView::Step(copy)),
        ));
      }
    })
    .into()
}

/// The copy of step `key` named `name`, so that it can be told from the step. The core's copy waits for what the step
/// waits for and nothing waits for it, which would leave a step that waits for only part of what runs before it, a
/// branch no single line shows: the steps that wait for the step wait for the copy too, so the copy runs beside it.
fn copy_of(edit: &mut PipelineEdit, key: &StepKey, name: &str) -> Option<StepKey> {
  let copy = edit.duplicate_step(key)?;
  // A name the copy cannot take (too long once ` copy` is added) leaves it the step's own name.
  if let Ok(name) = Name::new(name) {
    edit.update(&copy, |step| step.name = name);
  }
  let dependents: Vec<StepKey> = edit
    .steps()
    .iter()
    .filter(|step| step.key != copy && step.depends_on.contains(key))
    .map(|step| step.key.clone())
    .collect();
  for dependent in &dependents {
    edit.set_dependency(dependent, &copy, true);
  }
  Some(copy)
}
