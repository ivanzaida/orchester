//! Layout contracts of the pipeline page: its fields, New pipeline's starts, Versions and the Settings tab.
use orchester_core::store::ResourceStore;

use crate::pipelines_support::Editor;

#[test]
fn pipelines_compact_inspector_pointer_focus_covers_body_and_insets_and_saves_typed_text() {
  let mut editor = Editor::new();
  let id = editor.empty("Pointer focus");
  editor.open(id);
  for (typed_count, (horizontal, vertical)) in [
    (0.1, 0.2),
    (0.5, 0.7),
    (0.99, 0.2),
    (0.01, 0.5),
    (0.5, 0.03),
    (0.5, 0.97),
  ]
  .into_iter()
  .enumerate()
  {
    let bounds = editor
      .window
      .tree
      .get_element_by_id_mut("pipeline-description-box")
      .and_then(|element| element.bounds())
      .expect("summary field laid out");
    // The row's one-line field must not inherit the Tools 50px or ordinary 84px minimum.
    assert!(bounds.height >= 28.0 && bounds.height < 50.0, "{bounds:?}");
    editor.focus("pipeline-summary-name");
    let x = bounds.x + bounds.width * horizontal;
    let y = bounds.y + bounds.height * vertical;
    editor.window.tree.mouse_move(x, y);
    editor
      .window
      .tree
      .mouse_down(x, y, lurq::app::events::MouseButton::Left);
    editor.window.tree.mouse_up(x, y, lurq::app::events::MouseButton::Left);
    editor.window.pass();
    assert_eq!(
      editor.window.tree.focused_element().and_then(|element| element.id()),
      Some("pipeline-description"),
      "pointer at {horizontal},{vertical}"
    );
    editor.key("x", false);
    // The fixture has no description: each pointer focus adds one more letter.
    assert_eq!(editor.value("pipeline-description"), "x".repeat(typed_count + 1));
  }
  let typed = editor.value("pipeline-description");
  editor.click("pipeline-save");
  editor.until("the typed text saved", |current| {
    current.stored(id).description() == typed
  });
}

#[test]
fn pipelines_new_starter_row_keeps_its_steps_across_creation_states() {
  let mut editor = Editor::new();
  editor.empty("Existing pipeline");
  editor.go(orchester_app::routes::ProjectPage::NewPipeline);
  editor.until("new pipeline", |e| e.exists("pipeline-start-0"));
  for state in ["default", "collision", "empty"] {
    if state == "collision" {
      editor.set("pipeline-name", "Existing pipeline");
      editor.until("name collision", |e| e.exists("pipeline-open-existing"));
    } else if state == "empty" {
      editor.set("pipeline-name", "");
      editor.click("pipeline-start-1");
    }
    let chain = editor.text("pipeline-start-0-hint");
    assert!(chain.contains("Run checks ∥ Review"), "{state}: {chain}");
    let bounds = editor
      .window
      .tree
      .get_element_by_id_mut("pipeline-start-0")
      .and_then(|element| element.bounds())
      .expect("starter row laid out");
    // `Pane` `DuHgB`: rows 720 wide at most.
    assert!(bounds.width <= 720.5, "{state}: {bounds:?}");
  }
}

#[test]
fn pipelines_step_fields_fit_the_column_and_dependency_picker_stays_compact() {
  let mut editor = Editor::team();
  editor.profile();
  let id = editor.empty("Layout");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.choose("step-executor", "Reviewer");
  for id in ["step-executor", "step-model", "step-budget-box"] {
    let bounds = editor
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.bounds())
      .expect("field laid out");
    assert!(bounds.x + bounds.width <= 1432.0, "{id} clips: {bounds:?}");
    assert!(bounds.width <= 221.0, "{id} exceeds its row's control: {bounds:?}");
  }
  assert!(!editor.exists("step-after-menu"));
  editor.click("step-after-picker");
  assert!(editor.exists("step-after-menu"));
  editor.click("step-instructions-row");
  editor.set(
    "step-instructions",
    "Check the change against the acceptance criteria and explain the result.",
  );
  let bounds = editor
    .window
    .tree
    .get_element_by_id_mut("step-instructions-box")
    .and_then(|e| e.bounds())
    .expect("instructions laid out");
  assert!(bounds.x + bounds.width <= 1432.0, "instructions clip: {bounds:?}");
}

#[test]
fn pipelines_settings_save_the_next_version_and_history_shows_the_selected_one() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  assert!(editor.text("tab-1").contains("Delivery"));
  editor.click("pipeline-settings");
  editor.set("pipeline-settings-name", "Shipping");
  editor.click("pipeline-save");
  editor.until("name saved", |e| {
    e.db()
      .resource(id)
      .unwrap()
      .is_some_and(|r| r.name().as_str() == "Shipping")
  });
  editor.until("tab renamed", |e| e.text("tab-1").contains("Shipping"));
  assert_eq!(editor.db().resource(id).unwrap().unwrap().slug().as_str(), "delivery");
  editor.set("pipeline-settings-description", "Changed description");
  editor.click("pipeline-save");
  // A pipeline holds no draft (D-032): the description is saved as the next version.
  editor.until("description saved as v2", |e| {
    e.version(id) == 2 && e.stored(id).description() == "Changed description"
  });
  assert!(
    !editor.exists("pipeline-settings-change-strip"),
    "nothing is left unpublished to undo"
  );
  assert_eq!(editor.text("pipeline-on-start-value"), "On task start");
  assert!(
    !editor.exists("pipeline-settings-nav-starts"),
    "the planned sections are gone"
  );
  editor.click("pipeline-tab-steps");
  editor.click("pipeline-history");
  editor.until("both versions", |e| {
    e.exists("pipeline-version-1") && e.exists("pipeline-version-2")
  });
  editor.click("pipeline-version-1");
  editor.until("v1's changes", |e| e.exists("pipeline-version-summary"));
  assert!(editor.text("pipeline-version-summary").contains("Changes in v1"));
}

#[test]
fn pipelines_settings_preserves_invalid_name_across_navigation() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-settings");
  editor.set("pipeline-settings-name", "");
  assert!(editor.text("pipeline-settings-page").contains("Enter a pipeline name"));
  editor.click("pipeline-tab-steps");
  editor.click("pipeline-settings");
  assert_eq!(editor.value("pipeline-settings-name"), "");
  assert!(editor.text("pipeline-settings-page").contains("Enter a pipeline name"));
  editor.set("pipeline-settings-name", "Recovered");
  editor.click("pipeline-save");
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

  let mut editor = Editor::team();
  let id = editor.empty("Routes");
  editor.open(id);
  editor.remove_only_step();
  editor.click("pipeline-use-starter");
  let route = editor
    .window
    .tree
    .get_element_by_id_mut("pipeline-failure-route-review")
    .and_then(|element| element.bounds())
    .expect("review's route laid out");
  assert!(
    route.width > 0.0 && route.height > 0.0,
    "route line collapsed: {route:?}"
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
