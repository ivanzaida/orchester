//! Visual-regression contracts for the inspector and versioned Settings.
use orchester_core::{resource::Pipeline, store::ResourceStore};

use crate::pipelines_support::Editor;

#[test]
fn pipelines_inspector_fields_fit_the_window_and_dependency_picker_stays_compact() {
  let mut editor = Editor::new();
  editor.profile();
  let id = editor.empty("Layout");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.choose("step-executor", "Reviewer");
  editor.set(
    "step-instructions",
    "Check the change against the acceptance criteria and explain the result.",
  );
  for id in [
    "step-executor",
    "step-model",
    "step-instructions-box",
    "step-budget-box",
  ] {
    let bounds = editor
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.bounds())
      .expect("field laid out");
    assert!(bounds.x + bounds.width <= 1432.0, "{id} clips: {bounds:?}");
    assert!(bounds.width <= 184.0, "{id} exceeds value column: {bounds:?}");
  }
  for id in ["step-executor-row", "step-model-row", "step-budget-row"] {
    let bounds = editor
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .expect("compact row laid out");
    assert!(
      (bounds.height - 28.0).abs() < 0.1,
      "{id} adds vertical inset: {bounds:?}"
    );
  }
  assert!(!editor.exists("step-after-menu"));
  editor.click("step-after-picker");
  assert!(editor.exists("step-after-menu"));
}

#[test]
fn pipelines_settings_undo_and_history_show_the_selected_version() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  assert!(editor.text("tab-1").contains("Delivery"));
  editor.click("pipeline-settings");
  editor.set("pipeline-settings-name", "Shipping");
  editor.until("name saved", |e| {
    e.db()
      .resource(id)
      .unwrap()
      .is_some_and(|r| r.name().as_str() == "Shipping")
  });
  editor.until("tab renamed", |e| e.text("tab-1").contains("Shipping"));
  assert_eq!(editor.db().resource(id).unwrap().unwrap().slug().as_str(), "delivery");
  editor.set("pipeline-settings-description", "Changed description");
  editor.until("description saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .is_some_and(|d| d.body().description() == "Changed description")
  });
  assert!(editor.exists("pipeline-settings-draft"));
  editor.click("pipeline-description-undo");
  assert_eq!(editor.value("pipeline-settings-description"), "");
  editor.until("undo saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .is_none_or(|d| d.body().description().is_empty())
  });
  editor.set("pipeline-settings-description", "Only in the draft");
  editor.until("draft metadata saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .is_some_and(|d| d.body().description() == "Only in the draft")
  });
  editor.click("pipeline-settings-nav-starts");
  let starts = editor
    .window
    .tree
    .get_element_by_id_mut("pipeline-on-start")
    .and_then(|e| e.bounds())
    .unwrap();
  assert!(
    starts.y > 88.0 && starts.y < 872.0,
    "Starts section was revealed: {starts:?}"
  );
  editor.click("pipeline-back-steps");
  editor.click("pipeline-history");
  editor.until("version", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  editor.click("pipeline-compare");
  assert!(editor.text("screen-pipelines").contains("Compared with saved draft v2"));
  editor.click("pipeline-start");
  assert_eq!(editor.value("pipeline-description"), "What this pipeline is for");
}

#[test]
fn pipelines_settings_preserves_invalid_name_across_navigation() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-settings");
  editor.set("pipeline-settings-name", "");
  assert!(editor.text("pipeline-settings-page").contains("Enter a pipeline name"));
  editor.click("pipeline-back-steps");
  editor.click("pipeline-settings");
  assert_eq!(editor.value("pipeline-settings-name"), "");
  assert!(editor.text("pipeline-settings-page").contains("Enter a pipeline name"));
  editor.set("pipeline-settings-name", "Recovered");
  editor.until("valid name saved after remount", |e| {
    e.db()
      .resource(id)
      .expect("read resource")
      .is_some_and(|r| r.name().as_str() == "Recovered")
  });
  assert!(!editor.text("pipeline-settings-page").contains("Enter a pipeline name"));
}

#[test]
fn pipelines_failure_routes_are_visible_and_do_not_intercept_card_centers() {
  use lurq::app::events::MouseButton;

  let mut editor = Editor::new();
  let id = editor.empty("Routes");
  editor.open(id);
  editor.click("pipeline-use-starter");
  let route = editor
    .window
    .tree
    .get_element_by_id_mut("pipeline-failure-route-review")
    .and_then(|element| element.bounds())
    .expect("review return connector laid out");
  assert!(
    route.width > 0.0 && route.height > 0.0,
    "route overlay collapsed: {route:?}"
  );
  for (key, name) in [("run-checks", "Run checks"), ("review", "Review")] {
    let bounds = editor
      .window
      .tree
      .get_element_by_id_mut(&format!("pipeline-step-{key}"))
      .and_then(|element| element.bounds())
      .expect("parallel card laid out");
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    editor.window.tree.mouse_move(x, y);
    editor.window.tree.mouse_down(x, y, MouseButton::Left);
    editor.window.tree.mouse_up(x, y, MouseButton::Left);
    editor.window.pass();
    assert_eq!(editor.value("step-name"), name);
  }
}
