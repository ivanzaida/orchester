//! Fields (`Field/text` `JFXZg`, `Field/textarea` `s72p3L`, `Field/search` `Z7mGr2`), number fields with
//! their unit and read-back (`Limit field` `UQFos`) and selects (`Field/select` `pV2IA`).

use lurq::{app::ctx::Ctx, components::SelectOption, node::Element};
use orchester_app::ui::field::{FieldInput, NumberField, SelectField, field};

use super::{line, stack, wide_cell};
use crate::gallery::State;

/// A field's column in the gallery, and a select's (`hEEE0` is 180 wide).
const FIELD: f32 = 320.0;
const SELECT: f32 = 200.0;

pub fn section(ctx: &mut Ctx, state: &State) -> Element {
  let title = FieldInput::new("g-title", "Task title", state.title.clone())
    .placeholder("Add a task title")
    .render(ctx);
  let empty = FieldInput::new("g-empty", "Brief", state.outcome.clone())
    .placeholder("What should the run do?")
    .error("Write what the run should do.")
    .render(ctx);
  let outcome = FieldInput::new("g-outcome", "Describe the outcome", state.outcome.clone())
    .text_area()
    .placeholder("Describe the outcome")
    .render(ctx);
  let search = FieldInput::new("g-search", "Search tasks", state.search.clone())
    .search()
    .placeholder("Search title, key or brief")
    .render(ctx);
  let texts = [
    wide_cell(
      "Field/text · JFXZg",
      FIELD,
      field(
        "g-title-field",
        Some("Task title"),
        title,
        Some("Shown in lists and tabs."),
      ),
    ),
    wide_cell(
      "error · CqBFL",
      FIELD,
      field("g-empty-field", Some("Brief"), empty, None),
    ),
    wide_cell("Field/search · Z7mGr2", FIELD, search),
  ];
  let areas = [wide_cell(
    "Field/textarea · s72p3L",
    FIELD,
    field("g-outcome-field", Some("Describe the outcome"), outcome, None),
  )];
  stack([
    line(texts).into(),
    line(areas).into(),
    line(numbers(ctx, state)).into(),
    line(selects(ctx, state)).into(),
  ])
}

fn numbers(ctx: &mut Ctx, state: &State) -> [lurq::components::Column; 3] {
  let budget = NumberField::new(
    "g-budget",
    "Run budget",
    state.budget.clone(),
    "tokens",
    "= 1.4M tokens · input + output",
  )
  .placeholder("No limit")
  .grouped(",")
  .render(ctx);
  let minutes = NumberField::new(
    "g-minutes",
    "Time limit",
    state.minutes.clone(),
    "minutes",
    "No limit · the run takes the time it needs",
  )
  .placeholder("No limit")
  .render(ctx);
  let refused = NumberField::new("g-refused", "Run budget", state.minutes.clone(), "tokens", "")
    .placeholder("0")
    .error("A budget is at least 1 token.")
    .render(ctx);
  [
    wide_cell("Limit field · UQFos · tokens, grouped", FIELD, budget),
    wide_cell("Limit field · minutes, empty", FIELD, minutes),
    wide_cell("Limit field · not accepted", FIELD, refused),
  ]
}

fn selects(ctx: &Ctx, state: &State) -> [lurq::components::Column; 2] {
  let sort = SelectField::new(
    "g-sort",
    "Sort",
    state.sort.clone(),
    vec![
      SelectOption::new("manual", "Manual order"),
      SelectOption::new("updated", "Recently updated"),
      SelectOption::new("priority", "Priority"),
    ],
  )
  .width(lurq::node::dimension::Dimension::Px(SELECT))
  .render(ctx);
  let pipeline = SelectField::new(
    "g-pipeline",
    "Pipeline",
    state.pipeline.clone(),
    vec![
      SelectOption::new("feature", "Feature delivery v3").detail("Published v3 · 6 steps · approval at Plan"),
      SelectOption::new("quick", "Quick fix v2").detail("Published v2 · 3 steps · no approval gates"),
      SelectOption::new("draft", "Release v1")
        .detail("Drafts and pipelines never published are not listed.")
        .disabled(true),
    ],
  )
  .width(lurq::node::dimension::Dimension::Px(FIELD))
  .render(ctx);
  [
    wide_cell("Field/select · pV2IA", SELECT, sort),
    wide_cell("with details · ClKqz", FIELD, pipeline),
  ]
}
