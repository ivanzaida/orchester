//! The project's default pipeline (ORC-106): made from the Pipelines hub's panel `…` menu (PV2-12), shown with a
//! *Default* tag on its row and chip in its panel, stored with the project and chosen at first in New task; and
//! *Duplicate*, which makes a copy of it in the hub's panel.
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::id::ResourceId;

use crate::{pipelines_support::Editor, support::project_id};

/// Opens the hub on `id` and its panel's ⋯ menu.
fn open_menu(editor: &mut Editor, id: ResourceId) {
  editor.go(ProjectPage::Pipelines);
  editor.until("the list", |e| e.exists(&format!("pipeline-hub-row-{id}")));
  editor.click(&format!("pipeline-hub-row-{id}"));
  editor.until("the panel", |e| {
    e.exists("pipeline-hub-more") && e.text("team-hub-panel-title") == "Delivery"
  });
  editor.click("pipeline-hub-more");
  editor.until("the menu", |e| e.exists("pipeline-hub-menu-default"));
}

#[test]
fn pipelines_default_is_made_from_the_hub_menu_and_new_task_chooses_it() {
  let mut editor = Editor::new();
  let id = editor.published();
  let other = editor.empty("Other");
  open_menu(&mut editor, id);
  assert_eq!(
    editor.text("pipeline-hub-menu-default-label"),
    "Make default for New task"
  );
  assert!(editor.exists("pipeline-hub-menu-duplicate"));
  assert!(!editor.exists(&format!("pipeline-hub-row-{id}-tag")), "no default yet");
  editor.click("pipeline-hub-menu-default");
  editor.until("the Default tag on the row", |e| {
    e.exists(&format!("pipeline-hub-row-{id}-tag"))
  });
  assert_eq!(editor.text(&format!("pipeline-hub-row-{id}-tag")), "Default");
  assert_eq!(editor.db().default_pipeline().expect("the setting reads"), Some(id));
  assert!(
    !editor.exists(&format!("pipeline-hub-row-{other}-tag")),
    "only the default is tagged"
  );
  let mut chips = Vec::new();
  for index in 0..4 {
    let label = format!("pipeline-hub-chip-{index}-label");
    if editor.exists(&label) {
      chips.push(editor.text(&label));
    }
  }
  assert_eq!(
    chips.last().map(String::as_str),
    Some("Default"),
    "the panel's chips: {chips:?}"
  );
  editor.click("pipeline-hub-more");
  editor.until("the menu", |e| e.exists("pipeline-hub-menu-default"));
  assert_eq!(editor.text("pipeline-hub-menu-default-label"), "Default for New task");
  editor.click("pipeline-hub-menu-default");
  assert_eq!(
    editor.db().default_pipeline().expect("the setting reads"),
    Some(id),
    "the default row of the menu is inert"
  );
  editor
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::NewTask));
  editor
    .window
    .until("New task", |w| w.tree.get_element_by_id("task-form-pipeline").is_some());
  editor.window.until("the default chosen", |w| {
    w.texts("task-form-pipeline")
      .is_some_and(|texts| texts.join(" ").contains("Delivery"))
  });
}

#[test]
fn pipelines_duplicate_opens_new_pipeline_copying_the_pipeline() {
  let mut editor = Editor::new();
  let id = editor.published();
  open_menu(&mut editor, id);
  editor.click("pipeline-hub-menu-duplicate");
  // The copy is made in the hub's panel, unsaved (`Pipelines · v2` P14b).
  editor.until("New pipeline", |e| e.exists("pipeline-new-name"));
  assert!(
    matches!(
      editor.window.navigation().route_now(),
      Route::Project(_, ProjectPage::PipelinesHub(view)) if view.item.is_some_and(|shown| shown != id)
    ),
    "the hub shows the copy being made"
  );
  assert_eq!(editor.value("pipeline-new-name"), "Delivery copy");
  assert_eq!(
    editor.window.tree.focused_element().and_then(|element| element.id()),
    Some("pipeline-new-name"),
    "the name has the keys"
  );
  editor.key("x", false);
  assert_eq!(
    editor.value("pipeline-new-name"),
    "x",
    "the prefilled name is selected, so typing replaces it"
  );
  editor.set("pipeline-new-name", "Delivery copy");
  assert_eq!(editor.text("pipeline-new-start-copy-line-0"), "Published version → v1");
  assert_eq!(editor.text("team-hub-actions-hint"), "Saves Delivery copy as v1");
  editor.click("pipeline-new-create");
  editor.until("the copy opened", |e| {
    !e.exists("pipeline-new-panel") && e.exists("pipeline-hub-panel")
  });
  let copy = match editor.window.navigation().route_now() {
    Route::Project(_, ProjectPage::PipelinesHub(view)) => view.item.expect("the copy"),
    other => panic!("the hub: {other:?}"),
  };
  assert_ne!(copy, id);
  assert_eq!(editor.version(copy), 1, "the copy is published as its first version");
  assert_eq!(
    editor.stored(copy).steps()[0].name.as_str(),
    "Check",
    "v1 is the published version's copy"
  );
  assert_eq!(editor.version(id), 1, "the original is not touched");
}

/// What the project stores as its default is what New task chooses at first, whichever page made it the default.
#[test]
fn pipelines_default_stored_with_the_project_is_chosen_first_by_new_task() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor
    .db()
    .save_default_pipeline(Some(id))
    .expect("the setting is stored");
  editor
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::NewTask));
  editor
    .window
    .until("New task", |w| w.tree.get_element_by_id("task-form-pipeline").is_some());
  editor.window.until("the default chosen", |w| {
    w.texts("task-form-pipeline")
      .is_some_and(|texts| texts.join(" ").contains("Delivery"))
  });
}
