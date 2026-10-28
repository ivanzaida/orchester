//! The Limits chip (`Limits` `XKfLI`) and its popover: the run budget and the time limit (`Limit field` `UQFos`), each
//! read back in words under it. Each is optional and has no maximum (the person's decision of 2026-10-09): an empty
//! field is no limit, read back *No limit · the run uses what it needs*, and a task without either is complete and
//! runs; only text that is not a whole number is refused, in the field's own words. New task starts with the project's
//! defaults. The chip reads *Limits · Default* while the task holds them (`SclBL` in `cg0vn`), else what is set, e.g.
//! *Limits · 1.4M · 30 min*, or *Limits · No limit* when neither is, and says its name in `warning` while a value is
//! refused.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  limits::ProjectLimits,
  tokens::{compact_tokens, parse_optional_budget},
};
use orchester_workspace::{
  budget::{holds_defaults, read_minutes, whole_minutes},
  messages::Message,
};

use super::{
  chips::{ChipState, Look, PropertyChip},
  state::View,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{field::NumberField, icon::Icon, message::message_text, popover::popover_panel},
};

/// The popover's width.
const WIDTH: f32 = 460.0;

pub struct LimitFields<'a> {
  pub budget: &'a Signal<String>,
  pub time_limit: &'a Signal<String>,
  /// The project's limits, whose defaults the chip names *Default*.
  pub defaults: ProjectLimits,
}

pub fn limits_chip(
  ctx: &mut Ctx,
  view: &View,
  (state, fields): (&ChipState, &LimitFields<'_>),
  separator: &str,
) -> Element {
  let refused = view.budget_error.is_some() || view.time_error.is_some();
  let typed = (view.values.budget.as_str(), view.values.time_limit.as_str());
  let value = if refused {
    ctx.t("task_form.fields.limits.invalid")
  } else if holds_defaults(fields.defaults, typed, separator) {
    ctx.t("task_form.fields.limits.default")
  } else {
    chip_value(ctx, view, separator)
  };
  let panel = state.is_open().then(|| panel(ctx, view, fields, separator));
  PropertyChip::new(
    "task-form-limits",
    Icon::Gauge,
    (ctx.t("task_form.fields.limits.label"), value),
    Look::Empty,
  )
  .warning(refused)
  .render(state, panel)
}

/// What the chip says of valid limits.
fn chip_value(ctx: &Ctx, view: &View, separator: &str) -> Arc<str> {
  let tokens = parse_optional_budget(&view.values.budget, separator)
    .ok()
    .flatten()
    .map(|budget| compact_tokens(budget.get()));
  let minutes = read_minutes(&view.values.time_limit)
    .ok()
    .flatten()
    .map(|limit| match whole_minutes(limit) {
      1 => ctx.t("task_form.fields.limits.minutes.one"),
      count => ctx.t_args("task_form.fields.limits.minutes.other", [("count", count.to_string())]),
    });
  match (tokens, minutes) {
    (None, None) => ctx.t("task_form.fields.limits.none"),
    (Some(tokens), None) => ctx.t_args("task_form.fields.limits.tokens", [("tokens", tokens)]),
    (None, Some(minutes)) => minutes,
    (Some(tokens), Some(minutes)) => ctx.t_args(
      "task_form.fields.limits.both",
      [("tokens", tokens), ("minutes", minutes.to_string())],
    ),
  }
}

fn panel(ctx: &mut Ctx, view: &View, fields: &LimitFields<'_>, separator: &str) -> Element {
  let budget = budget_field(ctx, view, fields.budget, separator);
  let time = time_field(ctx, view, fields.time_limit);
  popover_panel("task-form-limits-popover", WIDTH)
    .child(Text::new(&ctx.t("task_form.fields.limits.title")).variant(ExtraTypography::InspectorSection.role()))
    .child(row(
      ctx,
      ("task_form.fields.budget.label", "task_form.fields.budget.description"),
      budget,
    ))
    .child(row(
      ctx,
      (
        "task_form.fields.timeLimit.label",
        "task_form.fields.timeLimit.description",
      ),
      time,
    ))
    .into()
}

/// A limit's name and what it is for, its field at the end.
fn row(ctx: &Ctx, (label, description): (&str, &str), field: Element) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(Text::new(&ctx.t(label)).variant(TypographyStyle::Label))
        .child(
          Text::new(&ctx.t(description))
            .width(Dimension::Pct(100.0))
            .variant(TypographyStyle::Caption),
        ),
    )
    .child(field)
}

fn budget_field(ctx: &mut Ctx, view: &View, value: &Signal<String>, separator: &str) -> Element {
  let read_back = match parse_optional_budget(&view.values.budget, separator) {
    Ok(None) => ctx.t("task_form.fields.budget.noLimitReadBack"),
    Ok(Some(budget)) => ctx.t_args(
      "task_form.fields.budget.readBack",
      [("tokens", compact_tokens(budget.get()))],
    ),
    // What the field refuses is said by its error.
    Err(_) => Arc::from(""),
  };
  let field = NumberField::new(
    "task-form-budget-input",
    ctx.t("task_form.fields.budget.label"),
    value.clone(),
    ctx.t("task_form.fields.budget.unit"),
    read_back,
  )
  .placeholder(ctx.t("task_form.fields.budget.noLimit"))
  .grouped(separator);
  finish(ctx, field, view, view.budget_error.as_ref())
}

fn time_field(ctx: &mut Ctx, view: &View, value: &Signal<String>) -> Element {
  let read_back = match read_minutes(&view.values.time_limit).map(|limit| limit.map(whole_minutes)) {
    Ok(None) => ctx.t("task_form.fields.timeLimit.noLimitReadBack"),
    Ok(Some(1)) => ctx.t("task_form.fields.timeLimit.readBack.one"),
    Ok(Some(minutes)) => ctx.t_args(
      "task_form.fields.timeLimit.readBack.other",
      [("count", minutes.to_string())],
    ),
    // What the field refuses is said by its error.
    Err(_) => Arc::from(""),
  };
  let field = NumberField::new(
    "task-form-time-limit-input",
    ctx.t("task_form.fields.timeLimit.label"),
    value.clone(),
    ctx.t("task_form.fields.timeLimit.unit"),
    read_back,
  )
  .placeholder(ctx.t("task_form.fields.timeLimit.noLimit"));
  finish(ctx, field, view, view.time_error.as_ref())
}

fn finish(ctx: &mut Ctx, field: NumberField, view: &View, error: Option<&Message>) -> Element {
  let field = match error {
    Some(error) => field.error(message_text(ctx, error)),
    None => field,
  };
  if view.locked() { field.locked() } else { field }.render(ctx)
}
