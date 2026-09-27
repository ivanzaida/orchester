//! The parts of the step inspector (`yCO8P`, `KKep6` when a profile is missing), top to bottom.
//! "Runs after" and Remove step are not in the design: dragging connectors is not built, so
//! dependencies are chosen here, and a step needs a way to be removed.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Select, SelectOption, Text},
  core::{Signal, SignalValue},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::resource::StepKind;

use super::{
  EditorHandle, Selection,
  step_card::step_icon,
  step_inspector::{FieldErrors, Fields, StepInspectorProps},
  step_permissions,
};
use crate::{
  messages::Message,
  pipelines::fields::{ModelChoice, RouteChoice, model_choices},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{ValueTone, field_error, select_style, select_trigger, styled_input, text_input, value_box_sized},
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{Ring, keyboard_stop},
    unit_input::UnitInput,
  },
};

/// Select and small button heights, instruction rows and the width of inline values
/// (`F4oXtQ`, `TCmqS`, `LmRHo`, `yeeIR`).
pub(super) const SELECT_HEIGHT: f32 = 36.0;
pub(super) const SMALL_BUTTON_HEIGHT: f32 = 30.0;
const INSTRUCTION_ROWS: (usize, usize) = (3, 12);
const VALUE_WIDTH: f32 = 96.0;

pub(super) fn label(ctx: &Ctx, key: &str) -> Text {
  Text::new(&ctx.t(key)).variant(TypographyStyle::Caption)
}

pub(super) fn labelled(label: Text, control: impl Into<Element>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Field.role())
    .child(label)
    .child(control)
}

pub(super) fn select<T: Clone + PartialEq + Send + Sync + SignalValue + 'static>(
  ctx: &Ctx,
  id: &'static str,
  value: &Signal<T>,
  options: impl IntoIterator<Item = impl Into<SelectOption<T>>>,
  required: bool,
) -> Element {
  let mut style = select_style(ctx, SELECT_HEIGHT);
  if required {
    style = style.trigger(select_trigger(ctx, SELECT_HEIGHT).border_inside(BorderSize::Sm, PaletteColor::TextPrimary));
  }
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .child(
      Select::new(value.clone())
        .options(options)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(SELECT_HEIGHT))
        .style(style),
    )
    .into()
}

/// Icon, the step's name as an editable title, and its number (`kXtai`).
pub fn header(ctx: &Ctx, props: &StepInspectorProps, fields: &Fields, error: Option<&Message>) -> Element {
  let name = styled_input(ctx, fields.name.clone(), TypographyStyle::Title)
    .id("step-name")
    .single_line()
    .flex(1.0);
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(icon(
      step_icon(&props.step),
      IconSize::Notice,
      PaletteColor::TextPrimary,
    ))
    .child(name)
    .child(
      Text::new(&ctx.t_args(
        "pipelines.inspector.stepNumber",
        [("number", format!("{:02}", props.number).as_str())],
      ))
      .variant(ExtraTypography::MonoTag.role()),
    );
  let mut column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(row);
  if let Some(error) = error {
    column = column.child(field_error(&message_text(ctx, error)));
  }
  column.into()
}

/// Model for this step and the effective model (`nje0G`, `LRvkv`); agent steps only.
pub fn model(ctx: &Ctx, props: &StepInspectorProps, fields: &Fields) -> Vec<Element> {
  let StepKind::Agent { profile, model, .. } = &props.step.kind else {
    return Vec::new();
  };
  let profile = profile.and_then(|id| props.assignables.profile(id));
  let Some(profile) = profile else {
    let unavailable = value_box_sized(
      ctx,
      "step-model",
      &ctx.t("pipelines.inspector.noProfileSelected"),
      ValueTone::Unavailable,
      None,
      Dimension::Px(SELECT_HEIGHT),
    );
    let effective = effective(
      ctx,
      &ctx.t("pipelines.inspector.noModelYet"),
      &ctx.t("pipelines.inspector.noModelYetDetail"),
      "pipelines.inspector.openProfiles",
    );
    return vec![
      labelled(label(ctx, "pipelines.inspector.model"), unavailable).into(),
      effective,
    ];
  };
  let from_profile = ctx.t_args("pipelines.inspector.fromProfile", [("model", profile.model.as_str())]);
  let mut options = vec![(ModelChoice(None), from_profile)];
  let models = model_choices(
    props.assignables.profiles.iter().map(|profile| &profile.model),
    model.as_ref(),
  );
  options.extend(models.into_iter().map(|model| {
    let label: Arc<str> = model.as_str().into();
    (ModelChoice(Some(model)), label)
  }));
  let used = model.as_ref().unwrap_or(&profile.model);
  let effective_model = match &profile.connection {
    Some(connection) => ctx.t_args(
      "pipelines.inspector.effectiveModel",
      [("connection", &**connection), ("model", used.as_str())],
    ),
    None => ctx.t_args(
      "pipelines.inspector.effectiveModelNoConnection",
      [("model", used.as_str())],
    ),
  };
  vec![
    labelled(
      label(ctx, "pipelines.inspector.model"),
      select(ctx, "step-model", &fields.model, options, false),
    )
    .into(),
    effective(
      ctx,
      &effective_model,
      &ctx.t("pipelines.inspector.overrideHint"),
      "pipelines.inspector.editProfile",
    ),
  ]
}

/// The effective model and the link to the profiles, which are not built yet (`LRvkv`).
fn effective(ctx: &Ctx, line: &str, hint: &str, button: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    // The full-width button's focus ring reaches past the column.
    .overflow_visible()
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(line)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
        .id("step-effective-model"),
    )
    .child(Text::new(hint).variant(ExtraTypography::Hint.role()))
    .child(
      ActionButton::new(ctx.t(button))
        .icon(Icon::ExternalLink)
        .id("step-edit-profile")
        .in_form()
        .fill_width()
        .height(Dimension::Px(SMALL_BUTTON_HEIGHT))
        .render(ctx),
    )
    .into()
}

/// Step instructions (`njgXh`), checked against the variable catalog as they are typed.
pub fn instructions(ctx: &Ctx, fields: &Fields, error: Option<&Message>) -> Element {
  let input = styled_input(ctx, fields.instructions.clone(), ExtraTypography::Instructions.role())
    .id("step-instructions")
    .textarea()
    .rows(INSTRUCTION_ROWS.0, INSTRUCTION_ROWS.1)
    .width(Dimension::Pct(100.0))
    .placeholder(&ctx.t("pipelines.inspector.instructionsPlaceholder"));
  let border = if error.is_some() {
    PaletteColor::BorderStrong
  } else {
    PaletteColor::Border
  };
  let area = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, border))
    .rounded(RadiusSize::Md)
    .child(input);
  let mut field = labelled(label(ctx, "pipelines.inspector.instructions"), area);
  if let Some(error) = error {
    field = field.child(field_error(&message_text(ctx, error)));
  }
  field.into()
}

/// The steps this one waits for, one toggle each.
pub fn runs_after(ctx: &Ctx, props: &StepInspectorProps, handle: &EditorHandle) -> Element {
  let others = props.steps.iter().filter(|step| step.key != props.step.key);
  let toggles = others.map(|other| {
    let on = props.step.depends_on.contains(&other.key);
    let (handle, step, dependency) = (handle.clone(), props.step.key.clone(), other.key.clone());
    toggle(other.name.as_str(), on, move || {
      handle.edit(|edit| edit.set_dependency(&step, &dependency, !on));
    })
  });
  let list = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(toggles);
  labelled(label(ctx, "pipelines.inspector.runsAfter"), list).into()
}

/// A checkbox-like row that toggles on click, or with Enter or Space once Tab reaches it.
fn toggle(text: &str, on: bool, flip: impl Fn() + Send + Sync + 'static) -> Element {
  let mark = if on { Icon::CircleCheck } else { Icon::Circle };
  let color = if on {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  keyboard_stop(Button::empty().button(), Ring::Inset)
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("checked", if on { "true" } else { "false" })
    .describe("step", text.to_owned())
    .cursor(CursorIcon::Pointer)
    .on_click(move |_| flip())
    .child(icon(mark, IconSize::Marker, color.clone()))
    .child(Text::new(text).variant(TypographyStyle::FieldLabel).color(color))
    .into()
}

/// When changes are requested (`ILUDx`): fail, or return to another step.
pub fn route(ctx: &Ctx, props: &StepInspectorProps, fields: &Fields) -> Element {
  let mut options = vec![(RouteChoice(None), ctx.t("pipelines.inspector.routeFail"))];
  options.extend(
    props
      .steps
      .iter()
      .filter(|step| step.key != props.step.key)
      .map(|step| {
        let label = ctx.t_args("pipelines.inspector.routeReturn", [("step", step.name.as_str())]);
        (RouteChoice(Some(step.key.clone())), label)
      }),
  );
  labelled(
    label(ctx, "pipelines.inspector.route"),
    select(ctx, "step-route", &fields.route, options, false),
  )
  .into()
}

/// Maximum retries, step budget and the step's permissions (`dSLqh`).
pub fn constraints(
  ctx: &Ctx,
  props: &StepInspectorProps,
  fields: &Fields,
  errors: &FieldErrors,
  handle: &EditorHandle,
) -> Element {
  let mut column = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Divider.role())
    .spacing(ExtraSpacing::Property.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()));
  if props.step.on_failure.is_some() {
    column = column.child(property_input(
      ctx,
      "pipelines.inspector.maxRetries",
      "step-retries",
      &fields.retries,
      errors.retries.as_ref(),
    ));
  }
  column
    .child(budget_input(ctx, fields, errors.budget.as_ref()))
    .with_children(step_permissions::rows(ctx, props, fields, handle))
    .into()
}

/// A labelled select on one line, as the step's repository access (`PFnPS`).
pub fn select_row<T: Clone + PartialEq + Send + Sync + SignalValue + 'static>(
  ctx: &Ctx,
  label: &str,
  id: &'static str,
  value: &Signal<T>,
  options: Vec<(T, Arc<str>)>,
) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(Text::new(label).variant(TypographyStyle::Caption).flex(1.0))
    .child(
      Column::new()
        .width(Dimension::Px(VALUE_WIDTH + VALUE_WIDTH / 2.0))
        .child(select(ctx, id, value, options, false)),
    )
    .into()
}

/// The step budget (`Jufa2`): whole tokens typed in place with the `tokens` unit inside the field
/// (`sN3du`), grouped for the locale while typed; empty for no step budget.
fn budget_input(ctx: &Ctx, fields: &Fields, error: Option<&Message>) -> Element {
  let input = UnitInput::new(
    "step-budget",
    fields.budget.clone(),
    ctx.t("tokens.unit"),
    fields.focused.clone(),
  )
  .tokens(ctx.t("number.groupSeparator"))
  .compact()
  .invalid(error.is_some())
  .size(
    Dimension::Px(VALUE_WIDTH + VALUE_WIDTH / 2.0),
    Dimension::Px(SELECT_HEIGHT),
  );
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(label(ctx, "pipelines.inspector.budget").flex(1.0))
    .child(input.render(ctx));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(row)
    .with_children(error.map(|error| field_error(&message_text(ctx, error))))
    .into()
}

/// A key/value property (`tfGRt`) whose value is typed in place.
fn property_input(ctx: &Ctx, key: &str, id: &'static str, value: &Signal<String>, error: Option<&Message>) -> Element {
  let input = text_input(
    ctx,
    id,
    value.clone(),
    error.is_some(),
    ExtraTypography::MonoValue.role(),
  )
  .width(Dimension::Px(VALUE_WIDTH))
  .height(Dimension::Px(SMALL_BUTTON_HEIGHT))
  .placeholder(&ctx.t("pipelines.inspector.none"));
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(label(ctx, key).flex(1.0))
    .child(input);
  let mut column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(row);
  if let Some(error) = error {
    column = column.child(field_error(&message_text(ctx, error)));
  }
  column.into()
}

/// Whether a person approves the step's result before the steps after it start (`s6Bxnp`).
/// A button, so Tab reaches it and Enter or Space switches it.
pub fn approval(ctx: &Ctx, props: &StepInspectorProps, handle: &EditorHandle) -> Element {
  let on = props.step.approval_gate;
  let (handle, key) = (handle.clone(), props.step.key.clone());
  keyboard_stop(Button::empty().button(), Ring::Inset)
    .id("step-approval")
    .describe("checked", if on { "true" } else { "false" })
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Field.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .on_click(move |_| {
      handle.edit(|edit| edit.update(&key, |step| step.approval_gate = !on));
    })
    .child(icon(Icon::ShieldCheck, IconSize::Option, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("pipelines.inspector.approval"))
        .variant(ExtraTypography::Hint.role())
        .flex(1.0),
    )
    .child(icon(
      if on { Icon::CircleCheck } else { Icon::Circle },
      IconSize::Option,
      if on {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextSecondary
      },
    ))
    .into()
}

/// Duplicate step (`krsgr`) and Remove step, which asks once more.
pub fn actions(ctx: &Ctx, props: &StepInspectorProps, fields: &Fields, handle: &EditorHandle) -> Element {
  let (duplicate, key) = (handle.clone(), props.step.key.clone());
  let duplicate_button = ActionButton::new(ctx.t("pipelines.inspector.duplicate"))
    .icon(Icon::Copy)
    .id("step-duplicate")
    .in_form()
    .fill_width()
    .on_click(move || {
      if let Some(Some(copy)) = duplicate.edit(|edit| edit.duplicate_step(&key)) {
        duplicate.select(Selection::Step(copy));
      }
    })
    .render(ctx);
  let confirming = fields.confirm_remove.get();
  let (remove, confirm, key) = (handle.clone(), fields.confirm_remove.clone(), props.step.key.clone());
  let label = if confirming {
    ctx.t_args(
      "pipelines.inspector.confirmRemove",
      [("step", props.step.name.as_str())],
    )
  } else {
    ctx.t("pipelines.inspector.remove")
  };
  let remove_button = ActionButton::new(label)
    .icon(Icon::Trash)
    .id("step-remove")
    .in_form()
    .fill_width()
    .on_click(move || {
      if confirm.get() {
        remove.edit(|edit| edit.remove_step(&key));
        remove.select(Selection::Pipeline);
      } else {
        confirm.set(true);
      }
    })
    .render(ctx);
  Column::new()
    .width(Dimension::Pct(100.0))
    // The full-width buttons' focus rings reach past the column.
    .overflow_visible()
    .spacing(SpacingSize::Sm)
    .child(duplicate_button)
    .child(remove_button)
    .into()
}
