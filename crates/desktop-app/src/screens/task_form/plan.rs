//! The Pipeline section (`Section · Pipeline` `cE6EO`): the select of the project's published pipelines
//! (`DQAHc`: drafts are not offered; without a published pipeline the select stays in place, unavailable
//! with its reason, and *Open Pipelines* leads to them), and the chosen pipeline's execution plan
//! (`Execution plan` `U5hUUe`): its steps with who runs each and the approval gates (`Plan step row`
//! `zA6TT`), and a note on the gates. Without a choice the plan's place says to choose (`No plan`
//! `j1DUN`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ResourceId;
use orchester_workspace::tasks::data::{PipelineChoice, PipelineStepChoice};

use super::{
  Nav,
  sections::{Label, Place, section, side_row},
  state::View,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField},
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The select's width (`Control` `ZIliW`), the plan's head and note heights (`Head` `FuYHQ`, `Approval
/// note` `QNkoh`), a step row's height (`Step 01` `bgxmZ`), and the columns of its number and name.
const SELECT_WIDTH: f32 = 240.0;
const PLAN_HEAD_HEIGHT: f32 = 36.0;
const STEP_HEIGHT: f32 = 34.0;
const STEP_NUMBER_WIDTH: f32 = 18.0;
const STEP_NAME_WIDTH: f32 = 150.0;

/// The chosen pipeline, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick(pub Option<ResourceId>);

impl lurq::app::component::DevtoolsInspectable for Pick {}

pub fn pipeline_section(ctx: &mut Ctx, view: &View, pick: &Signal<Pick>, nav: &Nav) -> Element {
  let note = view.queued().then(|| ctx.t("task_form.queued.lockedMeta"));
  let label = Label::new("task-form-pipeline", ctx.t("task_form.fields.pipeline.label"))
    .description(ctx.t("task_form.fields.pipeline.description"));
  let control = control(ctx, view, pick, nav);
  let plan = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Xs))
    .child(match view.pipeline() {
      Some(choice) => plan(ctx, choice, nav),
      None => no_plan(ctx),
    });
  let rows = vec![side_row(label, control, Place::Within, false), plan.into()];
  section(
    "task-form-pipeline-section",
    &ctx.t("task_form.sections.pipeline"),
    note.as_deref(),
    rows,
  )
  .into()
}

fn choice_label(ctx: &Ctx, choice: &PipelineChoice) -> Arc<str> {
  ctx.t_args(
    "task_form.plan.choice",
    [
      ("name", choice.name.to_string()),
      ("version", choice.version.to_string()),
    ],
  )
}

fn control(ctx: &mut Ctx, view: &View, pick: &Signal<Pick>, nav: &Nav) -> Element {
  let name = ctx.t("task_form.fields.pipeline.label");
  if view.no_pipelines() {
    let box_text = Signal::new(ctx.t("task_form.fields.pipeline.none").to_string());
    let nav = nav.clone();
    let open = ActionButton::new(
      "task-form-open-pipelines",
      ButtonKind::Ghost,
      ctx.t("task_form.fields.pipeline.open"),
    )
    .on_click(move || nav.pipelines());
    return Column::new()
      .width(Dimension::Px(SELECT_WIDTH))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::End)
      .child(
        FieldInput::new("task-form-pipeline-input", name, box_text)
          .locked()
          .trailing(Icon::Lock)
          .render(ctx),
      )
      .child(open)
      .into();
  }
  if view.locked() {
    let chosen = view
      .pipeline()
      .map(|choice| choice_label(ctx, choice))
      .unwrap_or_default();
    return Column::new()
      .width(Dimension::Px(SELECT_WIDTH))
      .child(
        FieldInput::new("task-form-pipeline-input", name, Signal::new(chosen.to_string()))
          .locked()
          .trailing(Icon::Lock)
          .render(ctx),
      )
      .into();
  }
  let options = view
    .pipelines
    .iter()
    .flat_map(|pipelines| pipelines.iter())
    .map(|choice| {
      SelectOption::new(Pick(Some(choice.id)), choice_label(ctx, choice)).detail(option_detail(ctx, choice))
    })
    .collect();
  SelectField::new("task-form-pipeline-input", name, pick.clone(), options)
    .placeholder(ctx.t("task_form.fields.pipeline.placeholder"))
    .width(Dimension::Px(SELECT_WIDTH))
    .render(ctx)
}

/// What an option says under its name: its version, its steps and where it waits for approval.
fn option_detail(ctx: &Ctx, choice: &PipelineChoice) -> Arc<str> {
  let steps = steps_text(ctx, choice.steps.len());
  let version = choice.version.to_string();
  match gates(ctx, &choice.steps) {
    Some(gates) => ctx.t_args(
      "task_form.plan.optionGates",
      [("version", version.as_str()), ("steps", &steps), ("gates", &gates)],
    ),
    None => ctx.t_args(
      "task_form.plan.optionNoGates",
      [("version", version.as_str()), ("steps", &steps)],
    ),
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

fn plan(ctx: &mut Ctx, choice: &PipelineChoice, nav: &Nav) -> Element {
  let nav = nav.clone();
  let view_pipeline = ActionButton::new(
    "task-form-view-pipeline",
    ButtonKind::Ghost,
    ctx.t("task_form.plan.view"),
  )
  .on_click(move || nav.pipelines());
  let meta = ctx.t_args(
    "task_form.plan.meta",
    [
      ("pipeline", choice_label(ctx, choice).to_string()),
      ("steps", steps_text(ctx, choice.steps.len())),
    ],
  );
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PLAN_HEAD_HEIGHT))
    .padding(Padding::new().left(SpacingSize::Md).right(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&ctx.t("task_form.plan.title")).variant(TypographyStyle::Label))
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&meta)
        .id("task-form-plan-meta")
        .variant(TypographyStyle::Caption),
    )
    .child(view_pipeline);
  let steps: Vec<Element> = (1..)
    .zip(&choice.steps)
    .map(|(number, step)| step_row(ctx, number, step))
    .collect();
  Column::new()
    .id("task-form-plan")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .child(head)
    .with_children(steps)
    .child(gate_note(ctx, &choice.steps))
    .into()
}

/// One step (`Plan step row` `zA6TT`): its number, name, who runs it and whether it waits for approval.
fn step_row(ctx: &Ctx, number: usize, step: &PipelineStepChoice) -> Element {
  let executor = Row::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(
      if step.tool { Icon::Terminal } else { Icon::Bot },
      IconSize::Base,
      PaletteColor::TextSecondary,
    ))
    .child(Text::new(&message_text(ctx, &step.executor)).variant(TypographyStyle::Description));
  let gate = step.approval_gate.then(|| {
    Row::new()
      .id(format!("task-form-step-{number}-gate"))
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::Hand, IconSize::Base, PaletteColor::Warning))
      .child(
        Text::new(&ctx.t("task_form.plan.gate"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::Warning)
          .nowrap(),
      )
  });
  Row::new()
    .id(format!("task-form-step-{number}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STEP_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&format!("{number:02}"))
        .width(Dimension::Px(STEP_NUMBER_WIDTH))
        .variant(ExtraTypography::MonoNote.role()),
    )
    .child(
      Text::new(&step.name)
        .width(Dimension::Px(STEP_NAME_WIDTH))
        .variant(TypographyStyle::Label),
    )
    .child(executor)
    .with_children(gate)
    .into()
}

/// Under the steps: which steps need approval (`Approval note` `QNkoh`), or that none does (`w6hmb`).
fn gate_note(ctx: &Ctx, steps: &[PipelineStepChoice]) -> Row {
  let (glyph, color, words) = match gates(ctx, steps) {
    Some(gated) => (
      Icon::Hand,
      PaletteColor::Warning,
      ctx.t_args("task_form.plan.approval", [("steps", gated)]),
    ),
    None => (Icon::Info, PaletteColor::TextMuted, ctx.t("task_form.plan.noGates")),
  };
  Row::new()
    .id("task-form-plan-note")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PLAN_HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color))
    .child(Text::new(&words).variant(TypographyStyle::Description))
}

/// The plan's place while no pipeline is chosen (`No plan` `j1DUN`).
fn no_plan(ctx: &Ctx) -> Element {
  Row::new()
    .id("task-form-no-plan")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(icon(Icon::Workflow, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("task_form.plan.choose"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .into()
}
