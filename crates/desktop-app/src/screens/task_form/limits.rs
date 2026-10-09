//! The Limits section (`Section · Limits` `WpoKn`): the run budget and the time limit (`Limit field`
//! `UQFos`), each read back in words under it. Both are optional and have no maximum (the person's
//! decision of 2026-10-09): an empty field is no limit, and only text that is not a whole number is
//! refused, in the field's own words (`i4Y9hL`).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::tokens::{compact_tokens, parse_budget};
use orchester_workspace::{budget::unbounded, messages::Message};

use super::{
  sections::{Label, Place, section, side_row},
  state::View,
};
use crate::ui::{field::NumberField, message::message_text};

pub struct LimitFields<'a> {
  pub budget: &'a Signal<String>,
  pub time_limit: &'a Signal<String>,
}

pub fn limits_section(ctx: &mut Ctx, view: &View, fields: &LimitFields<'_>, separator: &str) -> Element {
  let note = view.queued().then(|| ctx.t("task_form.queued.lockedMeta"));
  let budget = budget_field(ctx, view, fields.budget, separator);
  let time = time_field(ctx, view, fields.time_limit);
  let rows = vec![
    side_row(
      Label::new("task-form-budget", ctx.t("task_form.fields.budget.label"))
        .description(ctx.t("task_form.fields.budget.description")),
      budget,
      Place::Within,
      true,
    ),
    side_row(
      Label::new("task-form-time-limit", ctx.t("task_form.fields.timeLimit.label"))
        .description(ctx.t("task_form.fields.timeLimit.description")),
      time,
      Place::Last,
      true,
    ),
  ];
  section(
    "task-form-limits",
    &ctx.t("task_form.sections.limits"),
    note.as_deref(),
    rows,
  )
  .into()
}

fn budget_field(ctx: &mut Ctx, view: &View, value: &Signal<String>, separator: &str) -> Element {
  let text = view.values.budget.trim();
  let read_back = if text.is_empty() {
    ctx.t("task_form.fields.budget.noLimit")
  } else {
    parse_budget(text, separator, unbounded()).map_or_else(
      |_| Arc::from(""),
      |budget| {
        ctx.t_args(
          "task_form.fields.budget.readBack",
          [("tokens", compact_tokens(budget.get()))],
        )
      },
    )
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
  let text = view.values.time_limit.trim();
  let read_back = match text.parse::<u64>() {
    _ if text.is_empty() => ctx.t("task_form.fields.timeLimit.noLimit"),
    Ok(1) => ctx.t("task_form.fields.timeLimit.readBack.one"),
    Ok(minutes) => ctx.t_args(
      "task_form.fields.timeLimit.readBack.other",
      [("count", minutes.to_string())],
    ),
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
