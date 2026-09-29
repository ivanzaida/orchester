//! Right column of the new task form (`AoFj6`): pipeline, its steps, limits and the actions.
//! Only published pipelines can be chosen; with none, the selector says why it is empty.
//! Create & run needs every detail; while it works it says so, and when the run does not start
//! it lists why, keeping the form so the same run can be retried.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, ScrollVertical, Select, Spacer, Text, TextOverflow},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{limits::ProjectLimits, task::MissingDetail, tokens::compact_tokens};

use super::{FormState, NewTaskForm, PipelineSelection, missing_note};
use crate::{
  budget::read_budget,
  navigation::route::{Page, PipelineView},
  tabs::OpenIn,
  tasks::data::PipelineChoice,
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    button::{ActionButton, SKIPPED_BY_TAB},
    field::{ValueTone, select_style, value_box_sized, wrapping_field_error},
    form::Submit,
    icon::{Icon, IconSize, icon},
    link::Link,
    message_text,
    pressable::{Ring, keyboard_stop},
    unit_input::UnitInput,
  },
};

/// Column width, pipeline selector, step row, limit input and Create & run sizes (`AoFj6`,
/// `uhQv7`, `XVkaB`, `BVuOd`, `Hc1Cp`).
const PLAN_WIDTH: f32 = 336.0;
const SELECTOR_HEIGHT: f32 = 42.0;
const STEP_HEIGHT: f32 = 34.0;
const LIMIT_INPUT_WIDTH: f32 = 150.0;
const LIMIT_INPUT_HEIGHT: f32 = 32.0;
const CREATE_AND_RUN_HEIGHT: f32 = 42.0;

pub fn execution_plan(
  ctx: &Ctx,
  form: &NewTaskForm,
  (pipelines, limits): (Option<&Vec<PipelineChoice>>, ProjectLimits),
  state: &FormState,
  save: Submit,
  start: Submit,
) -> Element {
  let selected = form.pipeline.get().0;
  let pipeline = state
    .queued_task
    .as_ref()
    .map(|queued| queued.pipeline.as_ref())
    .or(state.starting_pipeline.as_deref())
    .or_else(|| pipelines.and_then(|choices| choices.iter().find(|choice| Some(choice.id) == selected)));
  let steps = pipeline.map(|choice| pipeline_steps(ctx, choice));
  let details = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(plan_heading(ctx))
    .child(pipeline_selector(
      ctx,
      &form.pipeline,
      pipelines,
      state
        .queued_task
        .as_ref()
        .map(|queued| queued.pipeline.as_ref())
        .or(state.starting_pipeline.as_deref()),
    ))
    .with_children(steps)
    .with_children(pipeline.map(|choice| view_pipeline(ctx, form, choice)))
    .with_children(missing_note(ctx, state, MissingDetail::Pipeline))
    .child(run_limits(ctx, form, state, limits))
    .with_children(pipeline.map(|choice| review_policy(ctx, choice)));
  Column::new()
    .id("execution-plan")
    .width(Dimension::Px(PLAN_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding_left(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Md)
    // The full-width actions at its edges keep their focus rings (`xURvJ`).
    .overflow_visible()
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      ScrollVertical::new(details)
        .id("execution-plan-details")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .child(actions(ctx, state, save, start))
    .into()
}

fn plan_heading(ctx: &Ctx) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("tasks.new.executionPlan")).variant(ExtraTypography::SectionHeading.role()))
    .child(Spacer::new().flex(1.0))
    .into()
}

fn view_pipeline(ctx: &Ctx, form: &NewTaskForm, pipeline: &PipelineChoice) -> Element {
  let link = keyboard_stop(
    Button::empty()
      .button()
      .id("view-pipeline")
      .padding_vertical(SpacingSize::Xs)
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .child(
        Row::new()
          .spacing(SpacingSize::Xs)
          .align_items(Alignment::Center)
          .child(Text::new(&ctx.t("tasks.new.viewPipeline")).variant(TypographyStyle::Link))
          .child(icon(Icon::ArrowUpRight, IconSize::Small, PaletteColor::TextPrimary)),
      ),
    Ring::Outer,
  );
  let nav = form.nav.clone();
  let page = Page::Pipeline(pipeline.id, PipelineView::Version(pipeline.version));
  // Keep the New Task form mounted while inspecting the selected pipeline.
  Link::new(form.nav.tabs(), move |open| {
    let open = if open == OpenIn::Current { OpenIn::NewTab } else { open };
    nav.open(page, open);
  })
  .attach(link)
  .into()
}

/// Pipeline selector (`uhQv7`): the project's published pipelines, or an unavailable box that
/// says why there is nothing to choose.
fn pipeline_selector(
  ctx: &Ctx,
  selection: &Signal<PipelineSelection>,
  pipelines: Option<&Vec<PipelineChoice>>,
  queued_pipeline: Option<&PipelineChoice>,
) -> Element {
  let height = Dimension::Px(SELECTOR_HEIGHT);
  if let Some(choice) = queued_pipeline {
    let label = ctx.t_args(
      "tasks.pipeline.published",
      [
        ("name", &*choice.name),
        ("version", choice.version.to_string().as_str()),
      ],
    );
    return value_box_sized(ctx, "pipeline-selector", &label, ValueTone::Value, None, height);
  }
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
  Element::from(
    Select::new(selection.clone())
      .options(options)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(SELECTOR_HEIGHT))
      .style(select_style(ctx, SELECTOR_HEIGHT)),
  )
  .id("pipeline-selector")
}

/// Steps of the chosen pipeline's published version (`o9ty9`).
fn pipeline_steps(ctx: &Ctx, pipeline: &PipelineChoice) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(pipeline.steps.iter().enumerate().map(|(index, step)| {
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
        .child(step_text(ctx, step).flex(1.0))
    }))
    .into()
}

fn step_text(ctx: &Ctx, step: &crate::tasks::data::PipelineStepChoice) -> Column {
  Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(&step.name)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(
      Text::new(&step_summary(ctx, step))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
}

fn step_summary(ctx: &Ctx, step: &crate::tasks::data::PipelineStepChoice) -> Arc<str> {
  let executor = message_text(ctx, &step.executor);
  if step.approval_gate {
    ctx.t_args("tasks.new.stepSummaryApproval", [("executor", &*executor)])
  } else {
    executor
  }
}

fn review_policy(ctx: &Ctx, pipeline: &PipelineChoice) -> Element {
  let gated = pipeline
    .steps
    .iter()
    .filter(|step| step.approval_gate)
    .map(|step| &*step.name)
    .collect::<Vec<_>>();
  let detail = match gated.as_slice() {
    [] => ctx.t("tasks.new.reviewPolicyNone"),
    [step] => ctx.t_args("tasks.new.reviewPolicyOne", [("step", *step)]),
    steps => ctx.t_args("tasks.new.reviewPolicyMany", [("steps", steps.join(", ").as_str())]),
  };
  Row::new()
    .id("review-policy")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::Hand, IconSize::Option, PaletteColor::TextSecondary))
    .child(
      Text::new(&detail)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextSecondary)
        .flex(1.0),
    )
    .into()
}

/// What Create & run is doing, why it cannot be pressed, or why the run did not start.
fn run_status(ctx: &Ctx, state: &FormState) -> Vec<Element> {
  if state.starting {
    return vec![
      Text::new(&ctx.t(if state.retrying {
        "tasks.new.retryingDetail"
      } else {
        "tasks.new.startingDetail"
      }))
      .variant(TypographyStyle::Caption)
      .into(),
    ];
  }
  if let Some(problems) = &state.run_problems {
    return problems
      .messages
      .iter()
      .map(|problem| wrapping_field_error(&message_text(ctx, problem)))
      .collect();
  }
  if state.recovered {
    return vec![
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Start)
        .child(icon(Icon::Clock3, IconSize::Small, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t("tasks.new.recoveredDetail"))
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextMuted)
            .flex(1.0),
        )
        .into(),
    ];
  }
  if state.missing.is_empty() {
    return Vec::new();
  }
  vec![
    Text::new(&ctx.t("tasks.new.runNeedsDetails"))
      .variant(TypographyStyle::Caption)
      .into(),
  ]
}

/// Run budget and time limit (`iuFln`), each with its read-back or its error (`RgvJX`, `d6zYDZ`).
fn run_limits(ctx: &Ctx, form: &NewTaskForm, state: &FormState, limits: ProjectLimits) -> Element {
  let budget_error = state.errors.budget.as_ref().map(|error| message_text(ctx, error));
  let time_error = state.errors.time_limit.as_ref().map(|error| message_text(ctx, error));
  let separator = ctx.t("number.groupSeparator");
  let max = limits.max_run_budget();
  let budget_readback = read_budget(&form.budget.get(), &separator, max).ok().map(|budget| {
    ctx.t_args(
      "tokens.readBackRun",
      [
        ("count", compact_tokens(budget.get()).as_str()),
        ("max", compact_tokens(max.get()).as_str()),
      ],
    )
  });
  let time_readback =
    (time_error.is_none() && !form.time_limit.get().trim().is_empty()).then(|| ctx.t("tasks.new.timeLimitReadBack"));
  let budget = UnitInput::new(
    "task-budget",
    form.budget.clone(),
    ctx.t("tokens.unit"),
    form.focused_input.clone(),
  )
  .tokens(separator)
  .compact()
  .read_only(state.queued || state.starting)
  .invalid(budget_error.is_some());
  let time = UnitInput::new(
    "task-time-limit",
    form.time_limit.clone(),
    ctx.t("tasks.new.minutes"),
    form.focused_input.clone(),
  )
  .compact()
  .read_only(state.queued || state.starting)
  .invalid(time_error.is_some());
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Step.role())
    .spacing(ExtraSpacing::Group.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(limit_row(ctx, "tasks.new.budget", budget))
    .with_children(budget_error.map(|error| wrapping_field_error(&error)))
    .with_children(budget_readback.map(|text| read_back("task-budget-readback", &text)))
    .child(limit_row(ctx, "tasks.new.timeLimit", time))
    .with_children(time_error.map(|error| wrapping_field_error(&error)))
    .with_children(time_readback.map(|text| read_back("task-time-readback", &text)))
    .with_children(missing_note(ctx, state, MissingDetail::Limits))
    .into()
}

/// A limit label beside its input (`hWeBp`).
fn limit_row(ctx: &Ctx, label: &str, input: UnitInput) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .justify(Justify::SpaceBetween)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t(label)).variant(TypographyStyle::FieldLabel))
    .child(
      input
        .size(Dimension::Px(LIMIT_INPUT_WIDTH), Dimension::Px(LIMIT_INPUT_HEIGHT))
        .render(ctx),
    )
    .into()
}

/// The value read back under a limit, right-aligned (`RgvJX`).
fn read_back(id: &'static str, text: &str) -> Element {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .justify(Justify::End)
    .child(
      Text::new(text)
        .variant(ExtraTypography::Hint.role())
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// Create & run with what it is doing or why it did not start, then Save as draft (`Hc1Cp`,
/// `Q4xE7q`). `save` and `start` are `None` while they cannot be pressed.
fn actions(ctx: &Ctx, state: &FormState, save: Submit, start: Submit) -> Element {
  let status: Option<Element> = state
    .save_error
    .as_ref()
    .map(|error| wrapping_field_error(&message_text(ctx, error)));
  let run_status = run_status(ctx, state);
  // Full width, so a long reason wraps inside the panel instead of running past its edge.
  let reasons: Vec<Text> = state
    .errors
    .save_reasons()
    .iter()
    .map(|reason| {
      Text::new(&message_text(ctx, reason))
        .variant(TypographyStyle::Caption)
        .width(Dimension::Pct(100.0))
    })
    .collect();
  let label = if state.starting {
    ctx.t("tasks.new.starting")
  } else if state.queued_task.is_some() {
    ctx.t("tasks.new.retryRun")
  } else {
    ctx.t("tasks.new.createAndRun")
  };
  let enabled = start.is_some();
  let create_and_run = ActionButton::new(label)
    .primary()
    .icon(if state.starting { Icon::Loader } else { Icon::Play })
    .id("create-and-run")
    .in_form()
    .fill_width()
    .height(Dimension::Px(CREATE_AND_RUN_HEIGHT))
    .on_click(move || {
      if let Some(start) = &start {
        start();
      }
    })
    .enabled(enabled);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .overflow_visible()
    .child(create_and_run.render(ctx))
    .with_children(run_status)
    .with_children(status)
    .with_children(reasons)
    .with_children((!state.queued).then(|| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .justify(Justify::Center)
        .child(save_button(ctx, state, save))
    }))
    .into()
}

/// Save as draft, or Save changes while a draft is edited (`Q4xE7q`, `fNWfv`); `save` is `None`
/// while it cannot be pressed.
fn save_button(ctx: &Ctx, state: &FormState, save: Submit) -> Button {
  let label = match (state.editing, state.saving) {
    (true, true) => ctx.t("tasks.edit.savingChanges"),
    (true, false) => ctx.t("tasks.edit.saveChanges"),
    (false, true) => ctx.t("tasks.new.savingDraft"),
    (false, false) => ctx.t("tasks.new.saveDraft"),
  };
  // Save changes reads in `$text` while it can be pressed (`PCXbi` in `ctHXE`), `$muted` while
  // it cannot (`fNWfv` in `E1o77O`).
  let color = match (state.editing, save.is_some()) {
    (true, true) => PaletteColor::TextPrimary,
    (true, false) => PaletteColor::TextSecondary,
    (false, _) => PaletteColor::TextMuted,
  };
  let text = Text::new(&label).variant(TypographyStyle::Caption).color(color);
  let save_draft = Button::empty()
    .button()
    .id("save-draft")
    .padding_horizontal(SpacingSize::Xs)
    .padding_vertical(SpacingSize::Xs)
    .align_items(Alignment::Start)
    .justify(Justify::Center)
    .rounded(RadiusSize::Sm)
    .child(text);
  let disabled = save.is_none();
  let save_draft = match save {
    Some(save) => keyboard_stop(save_draft, Ring::Outer)
      .describe("state", "enabled")
      .cursor(CursorIcon::Pointer)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .on_click(move |_| save()),
    None => save_draft
      .describe("state", "disabled")
      .cursor(CursorIcon::NotAllowed)
      .tab_index(SKIPPED_BY_TAB),
  };
  // Save changes that cannot be pressed fades (`fNWfv` in `E1o77O`).
  if state.editing && disabled {
    save_draft.opacity(DISABLED_OPACITY)
  } else {
    save_draft
  }
}
