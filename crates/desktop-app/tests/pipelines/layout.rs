//! Visual-regression contracts for the inspector and versioned Settings.
use orchester_core::{resource::Pipeline, store::ResourceStore};

use crate::pipelines_support::Editor;

#[test]
fn pipelines_compact_inspector_pointer_focus_covers_body_and_insets_and_saves_typed_text() {
  let mut editor = Editor::new();
  let id = editor.empty("Pointer focus");
  editor.open(id);
  for (horizontal, vertical) in [
    (0.1, 0.2),
    (0.5, 0.7),
    (0.99, 0.2),
    (0.01, 0.5),
    (0.5, 0.03),
    (0.5, 0.97),
  ] {
    let bounds = editor
      .window
      .tree
      .get_element_by_id_mut("pipeline-description-box")
      .and_then(|element| element.bounds())
      .expect("compact inspector field laid out");
    // The inspector's one-line field must not inherit the Tools 50px or ordinary 84px minimum.
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
    let before = editor
      .db()
      .draft::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .body()
      .description()
      .to_owned();
    editor.key("x", false);
    let typed = editor.value("pipeline-description");
    assert_eq!(typed.len(), before.len() + 1);
    editor.until("pointer edit saved", |current| {
      current
        .db()
        .draft::<Pipeline>(id)
        .unwrap()
        .unwrap()
        .body()
        .description()
        == typed
    });
  }
}

#[test]
fn pipelines_empty_library_entries_select_the_requested_start_on_the_creation_page() {
  for (action, empty) in [("pipeline-empty-create", false), ("pipeline-empty-start", true)] {
    let mut editor = Editor::new();
    editor.go(orchester_app::routes::ProjectPage::Pipelines);
    editor.until("empty library", |e| e.exists("pipelines-empty"));
    assert!(editor.exists("pipeline-empty-create") && editor.exists("pipeline-empty-start"));
    assert!(
      editor
        .text("pipelines-empty")
        .contains("Nothing runs until you publish it")
    );
    let parallel = editor.text("pipeline-starter-parallel");
    assert!(parallel.contains("Run checks") && parallel.contains("Review"));
    editor.click(action);
    editor.until("new pipeline page", |e| e.exists("pipeline-name"));
    let route = editor.window.navigation().route_now();
    assert_eq!(orchester_app::routes::Route::parse(&route.path()).unwrap(), route);
    assert!(
      editor
        .db()
        .resources(orchester_core::resource::ResourceKind::Pipeline)
        .unwrap()
        .is_empty()
    );
    editor.set("pipeline-name", "Entry choice");
    editor.click("pipeline-create");
    editor.until("created draft", |e| e.exists("pipeline-add-step"));
    let id = editor.selected();
    let draft = editor.db().draft::<Pipeline>(id).unwrap().unwrap();
    assert_eq!(draft.body().steps().is_empty(), empty);
    assert!(editor.db().latest_version::<Pipeline>(id).unwrap().is_none());
  }
}

#[test]
fn pipelines_new_starter_preview_stays_compact_across_creation_states() {
  let mut editor = Editor::new();
  editor.empty("Existing pipeline");
  editor.go(orchester_app::routes::ProjectPage::NewPipeline);
  editor.until("new pipeline", |e| e.exists("pipeline-start-0"));
  for state in ["default", "collision", "empty"] {
    if state == "collision" {
      editor.set("pipeline-name", "Existing pipeline");
      editor.until("name collision", |e| e.exists("pipeline-slug-collision"));
    } else if state == "empty" {
      editor.set("pipeline-name", "");
      editor.click("pipeline-start-2");
    }
    assert!(!editor.exists("pipeline-starter-parallel"));
    assert!(editor.text("pipeline-starter-parallel-marker").contains('∥'));
    let bounds = editor
      .window
      .tree
      .get_element_by_id_mut("pipeline-start-0")
      .and_then(|element| element.bounds())
      .expect("starter option laid out");
    // Iewpp in lAQis: two compact chip rows inside a 560 × 103 option.
    assert!((bounds.width - 560.0).abs() < 1.0, "{state}: {bounds:?}");
    assert!((bounds.height - 103.0).abs() < 3.0, "{state}: {bounds:?}");
  }
}

#[test]
fn pipelines_first_version_without_draft_has_no_phantom_entry_or_self_comparison() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-history");
  editor.until("first version", |e| e.exists("pipeline-version-1"));
  assert!(!editor.exists("pipeline-history-draft"));
  assert!(editor.text("pipeline-version-1").contains("first version"));
  editor.click("pipeline-version-1");
  assert_eq!(editor.text("pipeline-compare-label"), "Compare with draft");
  assert!(
    editor
      .text("pipeline-history-no-draft")
      .contains("Your next edit starts draft v2")
  );
  editor.click("pipeline-compare");
  assert!(!editor.exists("pipeline-comparison-changes"));
  assert!(!editor.text("screen-pipelines").contains("Compared with published"));
  for id in ["pipeline-compare", "pipeline-restore"] {
    let bounds = editor
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.bounds())
      .unwrap();
    assert!(
      bounds.x >= 1092.0 && bounds.x + bounds.width <= 1432.0,
      "{id}: {bounds:?}"
    );
  }
  assert!(editor.db().draft::<Pipeline>(id).unwrap().is_none());
  editor.click("pipeline-back-draft");
  editor.set("pipeline-description", "First unpublished edit");
  editor.until("real draft saved", |e| e.db().draft::<Pipeline>(id).unwrap().is_some());
  editor.click("pipeline-history");
  editor.until("real draft entry", |e| e.exists("pipeline-history-draft"));
  editor.click("pipeline-version-1");
  editor.click("pipeline-compare");
  assert!(editor.text("pipeline-comparison-changes").contains("saved draft v2"));
}

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
