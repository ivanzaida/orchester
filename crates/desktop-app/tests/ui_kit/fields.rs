//! Text fields, text areas, search, number fields with their unit and read-back, and selects.

use lurq::{
  app::theme::SpacingSize,
  components::{Column, SelectOption},
  core::Signal,
  node::dimension::Dimension,
};
use orchester_app::ui::{
  field::{FieldInput, NumberField, SelectField, field, field_error},
  icon::Icon,
};

use crate::harness::{Kit, assert_close, gap_after, gap_under};

/// The fields' column (`JFXZg` is 240 wide; New task's fields fill 720).
const COLUMN: f32 = 400.0;

fn form(value: &Signal<String>, area: &Signal<String>, search: &Signal<String>) -> Kit {
  let (value, area, search) = (value.clone(), area.clone(), search.clone());
  Kit::show(move |ctx| {
    let title = FieldInput::new("title", "Task title", value.clone())
      .placeholder("Add a task title")
      .render(ctx);
    let outcome = FieldInput::new("outcome", "Describe the outcome", area.clone())
      .text_area()
      .render(ctx);
    Column::new()
      .width(Dimension::Px(COLUMN))
      .spacing(SpacingSize::Lg)
      .child(field("title-field", Some("Task title"), title, Some("Shown in lists")))
      .child(field("outcome-field", Some("Describe the outcome"), outcome, None))
      .child(
        FieldInput::new("search", "Search tasks", search.clone())
          .search()
          .placeholder("Search")
          .render(ctx),
      )
      .into()
  })
}

#[test]
fn a_text_field_is_a_28_high_box_10_in_with_its_label_and_hint_6_away() {
  let value = Signal::new(String::new());
  let mut kit = form(&value, &Signal::new(String::new()), &Signal::new(String::new()));
  let (label, frame, input, hint) = (
    kit.bounds("title-field-label"),
    kit.bounds("title-box"),
    kit.bounds("title"),
    kit.bounds("title-field-hint"),
  );
  assert_close(frame.height, 28.0, "box height");
  assert_close(frame.width, COLUMN, "the box fills its column");
  assert_close(input.x - frame.x, 10.0, "value inset");
  assert_close(gap_under(label, frame), 6.0, "label to box");
  assert_close(gap_under(frame, hint), 6.0, "box to hint");
  assert_eq!(kit.texts("title-field-hint"), ["Shown in lists"]);
}

#[test]
fn a_text_area_is_84_high_at_least_7_over_its_lines() {
  let mut kit = form(
    &Signal::new(String::new()),
    &Signal::new(String::new()),
    &Signal::new(String::new()),
  );
  let (frame, input) = (kit.bounds("outcome-box"), kit.bounds("outcome"));
  assert_close(frame.height, 84.0, "text area height");
  assert_close(input.x - frame.x, 10.0, "sides");
  assert_close(input.y - frame.y, 7.0, "top");
}

#[test]
fn a_search_field_puts_its_glass_10_in_and_8_before_the_value() {
  let mut kit = form(
    &Signal::new(String::new()),
    &Signal::new(String::new()),
    &Signal::new(String::new()),
  );
  let (frame, glass, input) = (
    kit.bounds("search-box"),
    kit.bounds("search-icon"),
    kit.bounds("search"),
  );
  assert_close(frame.height, 28.0, "search height");
  assert_close(glass.x - frame.x, 10.0, "the glass's inset");
  assert_close(glass.height, 13.0, "the glass");
  assert_close(gap_after(glass, input), 8.0, "glass to value");
}

#[test]
fn a_field_typed_into_keeps_each_key_once() {
  let value = Signal::new(String::new());
  let mut kit = form(&value, &Signal::new(String::new()), &Signal::new(String::new()));
  kit.click("title");
  for key in ["a", "b", "c"] {
    kit.tree.key_down(
      key.to_owned(),
      format!("Key{}", key.to_uppercase()),
      false,
      false,
      false,
    );
    kit.pass();
  }
  assert_eq!(value.get(), "abc");
}

fn number(value: &Signal<String>, error: Option<&str>) -> Kit {
  let (value, error) = (value.clone(), error.map(str::to_owned));
  Kit::show(move |ctx| {
    let mut budget = NumberField::new(
      "budget",
      "Run budget",
      value.clone(),
      "tokens",
      "= 1.4M tokens · input + output",
    )
    .placeholder("No limit")
    .grouped(",");
    if let Some(error) = &error {
      budget = budget.error(error.clone());
    }
    Column::new().child(budget.render(ctx)).into()
  })
}

#[test]
fn a_number_field_is_200_wide_with_its_unit_inside_and_its_read_back_6_under_at_its_end() {
  let mut kit = number(&Signal::new(String::new()), None);
  let (frame, read_back) = (kit.bounds("budget-box"), kit.bounds("budget-read-back"));
  assert_eq!((frame.width, frame.height), (200.0, 28.0));
  assert_close(gap_under(frame, read_back), 6.0, "box to read-back");
  assert_close(
    frame.x + frame.width,
    read_back.x + read_back.width,
    "read-back at the end",
  );
  assert_eq!(kit.texts("budget-box").last().map(String::as_str), Some("tokens"));
  assert_eq!(kit.texts("budget-read-back"), ["= 1.4M tokens · input + output"]);
}

#[test]
fn a_token_count_is_grouped_while_it_is_typed() {
  let value = Signal::new(String::new());
  let mut kit = number(&value, None);
  kit.click("budget");
  for digit in "1400000".chars() {
    kit
      .tree
      .key_down(digit.to_string(), format!("Digit{digit}"), false, false, false);
    kit.pass();
  }
  assert_eq!(value.get(), "1,400,000");
}

#[test]
fn a_number_field_that_is_not_accepted_says_why_in_place_of_its_read_back() {
  let mut kit = number(&Signal::new("0".to_owned()), Some("A budget is at least 1 token."));
  assert!(!kit.exists("budget-read-back"));
  let (frame, note) = (kit.bounds("budget-box"), kit.bounds("budget-note"));
  assert_close(
    frame.x + frame.width,
    note.x + note.width,
    "the error at the box's end (`UQFos`)",
  );
  // Only the words, in `danger`, as the design draws them (`Aqupm` in `i4Y9hL`): no alert mark.
  assert_eq!(kit.texts("budget-note"), ["A budget is at least 1 token.".to_owned()]);
}

#[test]
fn a_select_is_a_28_high_field() {
  let chosen = Signal::new("manual");
  let mut kit = Kit::show(move |ctx| {
    Column::new()
      .child(
        SelectField::new(
          "sort",
          "Sort",
          chosen.clone(),
          vec![
            SelectOption::new("manual", "Manual order"),
            SelectOption::new("updated", "Recently updated").detail("Newest first"),
          ],
        )
        .width(Dimension::Px(160.0))
        .render(ctx),
      )
      .into()
  });
  let select = kit.bounds("sort");
  assert_eq!((select.width, select.height), (160.0, 28.0));
  assert!(kit.texts("sort").contains(&"Manual order".to_owned()));
}

#[test]
fn a_select_with_a_glyph_cuts_a_long_label_before_its_chevron() {
  let chosen = Signal::new("v1");
  let mut kit = Kit::show(move |ctx| {
    Column::new()
      .child(
        SelectField::new(
          "version",
          "Plan version",
          chosen.clone(),
          vec![
            SelectOption::new("v2", "v2 · latest"),
            SelectOption::new("v1", "v1 · changes requested, a long while ago"),
          ],
        )
        .leading(Icon::History)
        .width(Dimension::Px(170.0))
        .render(ctx),
      )
      .into()
  });
  let select = kit.bounds("version");
  assert_eq!((select.width, select.height), (170.0, 28.0));
  let label = kit.bounds("version-label");
  assert!(
    label.x + label.width <= select.x + select.width,
    "the label stays inside the box: {label:?}"
  );
}

#[test]
fn a_clearable_search_ends_in_a_cross_while_it_holds_text_and_the_cross_clears_it() {
  let value = Signal::new("retry budget".to_owned());
  let shown = value.clone();
  let mut kit = Kit::show(move |ctx| {
    Column::new()
      .child(
        FieldInput::new("search", "Search", shown.clone())
          .search()
          .clearable("Clear search")
          .width(lurq::node::dimension::Dimension::Px(300.0))
          .render(ctx),
      )
      .into()
  });
  let (frame, cross) = (kit.bounds("search-box"), kit.bounds("search-clear"));
  assert_close(
    frame.x + frame.width - (cross.x + cross.width),
    10.0,
    "the cross 10 from the end",
  );
  assert_close(cross.width, 13.0, "a 13 cross");
  kit.click("search-clear");
  assert_eq!(value.get(), "");
  assert!(!kit.exists("search-clear"), "an empty search has no cross");
}

#[test]
fn a_field_error_wraps_within_its_column() {
  let mut kit = Kit::show(|_ctx| {
    Column::new()
      .width(Dimension::Px(NARROW))
      .child(field_error(
        "refused",
        "The runner is not connected. Try again once it is online, then decide again.",
      ))
      .into()
  });
  let (error, text) = (kit.bounds("refused"), kit.bounds("refused-text"));
  assert_close(error.width, NARROW, "the error is as wide as its column");
  assert!(
    text.x + text.width <= error.x + error.width + 0.5,
    "the words end inside the column"
  );
  assert!(text.height > 20.0, "the words wrap onto a second line: {}", text.height);
  assert_eq!(
    kit.texts("refused-text"),
    ["The runner is not connected. Try again once it is online, then decide again."]
  );
}

/// A narrow column, as the retries panel's card is (`j7DjNG`, 288 inside).
const NARROW: f32 = 200.0;
