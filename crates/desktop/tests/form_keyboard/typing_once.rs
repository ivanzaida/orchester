//! Text typed into a field with real keys goes in once per key. A field bound to a signal that its `on_input` also
//! set typed each key twice ("kon" read "kkoonn"), since lurq calls `on_input` before it applies the key; tests that
//! fill a field with `set_value` never pass through `on_input` and could not see it.
//! `cargo test -p orchester-desktop --test form_keyboard typing_once`.

use crate::{harness::Desktop, organization::seed_dependencies};

/// Clicks the field `id` as a person does, types `text` with real keys and checks each key went in once: an empty
/// field then reads `text`, and a filled one grew by as many characters as were typed.
fn assert_types_once(desktop: &mut Desktop, id: &str, text: &str) {
  desktop.wait_for(id);
  desktop.press(id);
  let before = desktop.value(id);
  desktop.type_text(text);
  let after = desktop.value(id);
  assert_eq!(
    after.chars().count(),
    before.chars().count() + text.chars().count(),
    "{id}: {before:?} read {after:?} after typing {text:?}"
  );
  if before.is_empty() {
    assert_eq!(after, text, "{id}");
  }
}

fn tasks_page(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
}

fn usage_page() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-usage");
  desktop.press("nav-usage");
  desktop
}

#[test]
fn typing_once_in_the_new_task_tag_search() {
  let mut desktop = Desktop::with_project();
  tasks_page(&mut desktop);
  desktop.wait_for("tasks-new-task");
  desktop.click("tasks-new-task");
  desktop.wait_for("task-tags-picker");
  desktop.press("task-tags-picker");
  assert_types_once(&mut desktop, "tag-search", "Desk");
}

#[test]
fn typing_once_in_the_task_filter_tag_search() {
  let mut desktop = Desktop::with_seeded_project(seed_dependencies);
  tasks_page(&mut desktop);
  desktop.wait_for("filter-tags");
  desktop.press("filter-tags");
  assert_types_once(&mut desktop, "filter-tag-search", "UX");
}

#[test]
fn typing_once_in_the_project_tag_name() {
  let mut desktop = Desktop::with_project();
  tasks_page(&mut desktop);
  desktop.wait_for("project-tags-manage");
  desktop.press("project-tags-manage");
  desktop.wait_for("project-tag-new");
  desktop.press("project-tag-new");
  assert_types_once(&mut desktop, "project-tag-name", "Client");
}

#[test]
fn typing_once_in_the_usage_custom_range() {
  let mut desktop = usage_page();
  desktop.wait_for("usage-period-3");
  desktop.press("usage-period-3");
  assert_types_once(&mut desktop, "usage-custom-from", "1");
  assert_types_once(&mut desktop, "usage-custom-to", "2");
}

#[test]
fn typing_once_in_the_usage_filter_search() {
  let mut desktop = usage_page();
  desktop.wait_for("usage-filter-model");
  desktop.press("usage-filter-model");
  assert_types_once(&mut desktop, "usage-filter-search", "gpt");
}

#[test]
fn typing_once_in_the_usage_warning_threshold() {
  let mut desktop = usage_page();
  desktop.wait_for("usage-limits-menu");
  desktop.press("usage-limits-menu");
  assert_types_once(&mut desktop, "usage-threshold", "5");
}
