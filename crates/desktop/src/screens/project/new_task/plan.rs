//! Right column of the new task form (`AoFj6`): pipeline, its steps, limits and the actions.
//! Only published pipelines can be chosen; with none, the selector says why it is empty.
//! Create & run is unavailable: nothing can run tasks yet.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Select, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::task::MissingDetail;

use super::{FormState, NewTaskForm, PipelineSelection, error_note, missing_note};
use crate::{
  tasks::{data::PipelineChoice, form::BUDGET_CURRENCY},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, SELECT_CHEVRON_SIZE},
  ui::{
    button::{ActionButton, SKIPPED_BY_TAB},
    field::{ValueTone, select_trigger, styled_input, value_box_sized},
    form::Submit,
    icon::Icon,
    message_text,
  },
};

/// Column width, pipeline selector, step row, limit input and Create & run sizes (`AoFj6`,
/// `uhQv7`, `XVkaB`, `BVuOd`, `Hc1Cp`).
const PLAN_WIDTH: f32 = 336.0;
const SELECTOR_HEIGHT: f32 = 42.0;
const STEP_HEIGHT: f32 = 34.0;
const LIMIT_INPUT_WIDTH: f32 = 124.0;
const LIMIT_INPUT_HEIGHT: f32 = 32.0;
const CREATE_AND_RUN_HEIGHT: f32 = 42.0;

pub fn execution_plan(
  ctx: &Ctx,
  form: &NewTaskForm,
  pipelines: Option<&Vec<PipelineChoice>>,
  state: &FormState,
  save: Submit,
) -> Element {
  let selected = form.pipeline.get().0;
  let steps = pipelines
    .and_then(|choices| choices.iter().find(|choice| Some(choice.id) == selected))
    .map(|choice| pipeline_steps(&choice.steps));
  Column::new()
    .id("execution-plan")
    .width(Dimension::Px(PLAN_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding_left(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Md)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("tasks.new.executionPlan")).variant(ExtraTypography::SectionHeading.role()))
    .child(pipeline_selector(ctx, &form.pipeline, pipelines))
    .with_children(steps)
    .with_children(missing_note(ctx, state, MissingDetail::Pipeline))
    .child(limits(ctx, form, state))
    .child(Spacer::new().flex(1.0))
    .child(actions(ctx, state, save))
    .into()
}

/// Pipeline selector (`uhQv7`): the project's published pipelines, or an unavailable box that
/// says why there is nothing to choose.
fn pipeline_selector(
  ctx: &Ctx,
  selection: &Signal<PipelineSelection>,
  pipelines: Option<&Vec<PipelineChoice>>,
) -> Element {
  let height = Dimension::Px(SELECTOR_HEIGHT);
  let reason = match pipelines {
    None => ctx.t("tasks.new.pipelinesLoading"),
    Some(choices) if choices.is_empty() => ctx.t("tasks.new.noPipelines"),
    Some(choices) => return pipeline_select(ctx, selection, choices),
  };
  value_box_sized(ctx, "pipeline-selector", &reason, ValueTone::Unavailable, None, height)
}

fn pipeline_select(ctx: &Ctx, selection: &Signal<PipelineSelection>, choices: &[PipelineChoice]) -> Element {
  let none: Arc<str> = ctx.t("tasks.pipeline.notChosen");
  let options = std::iter::once((PipelineSelection(None), none)).chain(choices.iter().map(|choice| {
    let label = ctx.t_args(
      "tasks.pipeline.published",
      [
        ("name", &*choice.name),
        ("version", choice.version.to_string().as_str()),
      ],
    );
    (PipelineSelection(Some(choice.id)), label)
  }));
  let chevron = ctx.theme().palette_color(PaletteColor::TextSecondary);
  Select::new(selection.clone())
    .options(options)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SELECTOR_HEIGHT))
    .style_with(|style| {
      style
        .trigger(select_trigger(ctx, SELECTOR_HEIGHT))
        .chevron_color(chevron)
        .chevron_size(SELECT_CHEVRON_SIZE)
    })
    .into()
}

/// Steps of the chosen pipeline's published version (`o9ty9`).
fn pipeline_steps(steps: &[Arc<str>]) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(steps.iter().enumerate().map(|(index, name)| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(STEP_HEIGHT))
        .spacing(ExtraSpacing::Stack.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(&format!("{:02}", index + 1))
            .variant(TypographyStyle::Mono)
            .color(PaletteColor::TextMuted),
        )
        .child(
          Text::new(name)
            .variant(TypographyStyle::FieldLabel)
            .color(PaletteColor::TextPrimary)
            .nowrap(),
        )
    }))
    .into()
}

/// Run budget and time limit (`iuFln`).
fn limits(ctx: &Ctx, form: &NewTaskForm, state: &FormState) -> Element {
  let budget_error = state.errors.budget.as_ref().map(|error| message_text(ctx, error));
  let time_error = state.errors.time_limit.as_ref().map(|error| message_text(ctx, error));
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Step.role())
    .spacing(ExtraSpacing::Group.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(limit_row(
      ctx,
      "tasks.new.budget",
      "task-budget",
      &form.budget,
      BUDGET_CURRENCY.code(),
      budget_error.is_some(),
    ))
    .with_children(budget_error.map(|error| error_note(&error)))
    .child(limit_row(
      ctx,
      "tasks.new.timeLimit",
      "task-time-limit",
      &form.time_limit,
      &ctx.t("tasks.new.minutes"),
      time_error.is_some(),
    ))
    .with_children(time_error.map(|error| error_note(&error)))
    .with_children(missing_note(ctx, state, MissingDetail::Limits))
    .into()
}

/// A limit label beside a small input with its unit (`hWeBp`).
fn limit_row(ctx: &Ctx, label: &str, id: &'static str, value: &Signal<String>, unit: &str, invalid: bool) -> Element {
  let input = ctx.theme().form().input.clone();
  let border = if invalid {
    PaletteColor::BorderStrong
  } else {
    input.border.clone()
  };
  let field = Row::new()
    .width(Dimension::Px(LIMIT_INPUT_WIDTH))
    .height(Dimension::Px(LIMIT_INPUT_HEIGHT))
    .padding_custom(input.padding)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(input.background.clone())
    .border(Border::inside(BorderSize::Sm, border))
    .rounded(input.radius)
    .child(
      styled_input(ctx, value.clone(), TypographyStyle::Caption)
        .id(id)
        .single_line()
        .flex(1.0)
        .caret_color(PaletteColor::TextPrimary),
    )
    .child(Text::new(unit).variant(TypographyStyle::Caption).nowrap());
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .justify(Justify::SpaceBetween)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t(label)).variant(TypographyStyle::FieldLabel))
    .child(field)
    .into()
}

/// Create & run, unavailable with its reason, then Save as draft (`Hc1Cp`, `Q4xE7q`).
/// `save` is `None` while the draft cannot be saved, including while it is being saved.
fn actions(ctx: &Ctx, state: &FormState, save: Submit) -> Element {
  let status: Option<Element> = state
    .save_error
    .as_ref()
    .map(|error| error_note(&message_text(ctx, error)));
  let enabled = save.is_some();
  let label = if state.saving {
    ctx.t("tasks.new.savingDraft")
  } else {
    ctx.t("tasks.new.saveDraft")
  };
  let text = Text::new(&label).variant(TypographyStyle::Caption).color(if enabled {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextMuted
  });
  let save_draft = Button::empty()
    .button()
    .id("save-draft")
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Xs)
    .align_items(Alignment::Start)
    .justify(Justify::Center)
    .rounded(RadiusSize::Sm)
    .child(text);
  let save_draft = match save {
    Some(save) => save_draft
      .describe("state", "enabled")
      .cursor(CursorIcon::Pointer)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .on_click(move |_| save()),
    None => save_draft
      .describe("state", "disabled")
      .cursor(CursorIcon::NotAllowed)
      .tab_index(SKIPPED_BY_TAB),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      ActionButton::new(ctx.t("tasks.new.createAndRun"))
        .primary()
        .icon(Icon::Play)
        .id("create-and-run")
        .fill_width()
        .height(Dimension::Px(CREATE_AND_RUN_HEIGHT))
        .render(ctx),
    )
    .child(Text::new(&ctx.t("tasks.new.runUnavailable")).variant(TypographyStyle::Caption))
    .with_children(status)
    .with_children(
      state
        .errors
        .save_reasons()
        .iter()
        .map(|reason| Text::new(&message_text(ctx, reason)).variant(TypographyStyle::Caption)),
    )
    .child(save_draft)
    .into()
}
