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
  assert!(!editor.exists("step-after-menu"));
  editor.click("step-after-picker");
  assert!(editor.exists("step-after-menu"));
}

#[test]
fn pipelines_settings_undo_and_history_show_the_selected_version() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-settings");
  editor.set("pipeline-settings-name", "Shipping");
  editor.until("name saved", |e| {
    e.db()
      .resource(id)
      .unwrap()
      .is_some_and(|r| r.name().as_str() == "Shipping")
  });
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
  assert!(editor.text("screen-pipelines").contains("Compared with draft"));
  editor.click("pipeline-start");
  assert_eq!(editor.value("pipeline-description"), "");
}
