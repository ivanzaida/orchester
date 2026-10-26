//! The Pipeline chip (`Pipeline` `Fam0J`, set `sxreX`) and its menu (`Pipeline menu` `GyjOt` in `02` `lnN0y`): the
//! project's published pipelines, drafts not offered, each with its steps and where it waits for approval, a check on
//! the chosen one, and *Manage pipelines* under a separator. Without a published pipeline the menu says so and still
//! leads to them; a shared task's pipeline this computer lacks is named with what saving does. Once a pipeline is
//! chosen, the plan line under the chips (`Plan line` `o27CHI`) reads its steps, where approval is needed and *View
//! pipeline ↗*, in place of the old execution plan card.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::{ProjectId, ResourceId};
use orchester_workspace::tasks::{
  data::{PipelineChoice, PipelineStepChoice},
  list::PipelineLabel,
};

use super::{
  Nav,
  chips::{ChipState, Look, MenuOption, PropertyChip},
  state::View,
};
use crate::{
  navigation::Navigation,
  routes::{PipelineView, ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    filter::filter_menu_panel,
    icon::{Icon, icon},
    menu::{MenuItem, separator},
    message::message_text,
    pressable::{Focus, opening},
    tooltip::Truncated,
  },
};

/// The menu's width (`Pipeline menu` `GyjOt`).
const MENU_WIDTH: f32 = 380.0;

/// The chosen pipeline, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick(pub Option<ResourceId>);

impl lurq::app::component::DevtoolsInspectable for Pick {}

pub(super) fn choice_label(ctx: &Ctx, choice: &PipelineChoice) -> Arc<str> {
  ctx.t_args(
    "task_form.plan.choice",
    [
      ("name", choice.name.to_string()),
      ("version", choice.version.to_string()),
    ],
  )
}

/// The chip; `warning` once the person tried to run without a pipeline.
pub fn pipeline_chip(
  ctx: &mut Ctx,
  view: &View,
  (state, nav): (&ChipState, &Nav),
  (pick, warning): (&Signal<Pick>, bool),
) -> Element {
  let (look, value) = match (view.pipeline(), &view.missing_pipeline) {
    (Some(choice), _) => (Look::Set, choice_label(ctx, choice)),
    (None, Some(slug)) if view.values.pipeline.is_none() => (Look::Set, slug.clone()),
    (None, _) if view.no_pipelines() => (Look::Empty, ctx.t("task_form.fields.pipeline.noneShort")),
    (None, _) => (Look::Empty, ctx.t("task_form.fields.pipeline.choose")),
  };
  let panel = state.is_open().then(|| menu(ctx, view, (state, nav), pick));
  PropertyChip::new(
    "task-form-pipeline",
    Icon::Workflow,
    (ctx.t("task_form.fields.pipeline.label"), value),
    look,
  )
  .warning(warning && look == Look::Empty)
  .locked(view.locked())
  .render(state, panel)
}

fn menu(ctx: &mut Ctx, view: &View, (state, nav): (&ChipState, &Nav), pick: &Signal<Pick>) -> Element {
  let chosen = view.values.pipeline;
  let mut panel = filter_menu_panel("task-form-pipeline-menu", MENU_WIDTH);
  if let Some(slug) = view.missing_pipeline.as_deref().filter(|_| chosen.is_none()) {
    panel = panel.child(missing(ctx, slug));
  }
  if view.no_pipelines() {
    panel = panel.child(
      MenuItem::unavailable(
        "task-form-pipeline-none",
        ctx.t("task_form.fields.pipeline.none"),
        ctx.t("task_form.fields.pipeline.noneShort"),
      )
      .render(ctx, &state.open),
    );
  }
  for (index, choice) in view.pipelines.iter().flat_map(|pipelines| pipelines.iter()).enumerate() {
    let (id, pick) = (choice.id, pick.clone());
    panel = panel.child(
      MenuOption {
        id: format!("task-form-pipeline-option-{index}"),
        icon: Icon::Workflow,
        title: choice_label(ctx, choice),
        help: Some(option_detail(ctx, choice)),
        chosen: chosen == Some(id),
      }
      .render(&state.open, move || pick.set(Pick(Some(id)))),
    );
  }
  let manage = MenuItem::opens(
    "task-form-manage-pipelines",
    ctx.t("task_form.fields.pipeline.manage"),
    nav.to_pipelines(),
  )
  .icon(Icon::ArrowUpRight)
  .in_form()
  .render(ctx, &state.open);
  panel.child(separator(ctx)).child(manage).into()
}

/// A shared task's pipeline that this computer has none of, and what saving does with it.
fn missing(ctx: &Ctx, slug: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(&message_text(
        ctx,
        &PipelineLabel::Missing { slug: Arc::from(slug) }.message(),
      ))
      .id("task-form-pipeline-missing")
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::Warning),
    )
    .child(
      Text::new(&ctx.t("task_form.fields.pipeline.missingKept"))
        .id("task-form-pipeline-missing-kept")
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// What a pipeline says under its name: its steps and where it waits for approval.
fn option_detail(ctx: &Ctx, choice: &PipelineChoice) -> Arc<str> {
  let steps = steps_text(ctx, choice.steps.len());
  match gates(ctx, &choice.steps) {
    Some(gates) => ctx.t_args(
      "task_form.plan.menuGates",
      [("steps", steps.as_str()), ("gates", &gates)],
    ),
    None => ctx.t_args("task_form.plan.menuNoGates", [("steps", steps.as_str())]),
  }
}

fn steps_text(ctx: &Ctx, count: usize) -> String {
  let key = if count == 1 {
    "task_form.plan.steps.one"
  } else {
    "task_form.plan.steps.other"
  };
  ctx.t_args(key, [("count", count.to_string())]).to_string()
}

/// The names of the steps that wait for approval, in order, joined by the list separator; `None` when
/// none does.
fn gates(ctx: &Ctx, steps: &[PipelineStepChoice]) -> Option<String> {
  let gated: Vec<&str> = steps
    .iter()
    .filter(|step| step.approval_gate)
    .map(|step| &*step.name)
    .collect();
  let separator = ctx.t("task_form.plan.gateSeparator");
  (!gated.is_empty()).then(|| gated.join(&separator))
}

/// The plan line under the chips once a pipeline is chosen (`Plan line` `o27CHI`): a 13 `route`, the steps in 12
/// `text_secondary`, where approval is needed in `warning`, and *View pipeline ↗* at the end, 8 apart.
pub fn plan_line(ctx: &mut Ctx, view: &View, project: ProjectId) -> Option<Element> {
  let choice = view.pipeline()?;
  let steps = choice
    .steps
    .iter()
    .map(|step| &*step.name)
    .collect::<Vec<_>>()
    .join(&ctx.t("task_form.plan.stepSeparator"));
  let steps = Truncated::new("task-form-plan-steps", steps, TypographyStyle::Description).render(ctx);
  let (gate, color) = match gates(ctx, &choice.steps) {
    Some(gates) => (
      ctx.t_args("task_form.plan.lineGates", [("gates", gates)]),
      PaletteColor::Warning,
    ),
    None => (ctx.t("task_form.plan.lineNoGates"), PaletteColor::TextMuted),
  };
  let route = Route::Project(project, ProjectPage::Pipeline(choice.id, PipelineView::Steps));
  let view_pipeline = Focus::Link
    .apply(opening(Navigation::of(ctx).to(route)))
    .id("task-form-view-pipeline")
    .flex_shrink(0.0)
    .child(
      Text::new(&ctx.t("task_form.plan.view"))
        .variant(TypographyStyle::Description)
        .nowrap(),
    );
  Some(
    Row::new()
      .id("task-form-plan")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Route, IconSize::Base, PaletteColor::TextMuted))
      .child(Row::new().flex_shrink(1.0).min_width(Dimension::Px(0.0)).child(steps))
      .child(
        Text::new(&gate)
          .id("task-form-plan-gate")
          .variant(TypographyStyle::Description)
          .color(color)
          .nowrap()
          .flex_shrink(0.0),
      )
      .child(Spacer::new().flex(1.0))
      .child(view_pipeline)
      .into(),
  )
}
