//! The project's default pipeline (ORC-106): made from the Pipelines hub's panel `…` menu (PV2-12), shown with a
//! *Default* tag on its row and chip in its panel, stored with the project and chosen at first in New task; and
//! *Duplicate*, which opens New pipeline copying it.
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{id::ResourceId, resource::Pipeline, store::ResourceStore};

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
  let draft = editor.empty("Draft only");
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
    !editor.exists(&format!("pipeline-hub-row-{draft}-tag")),
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
fn pipelines_a_draft_only_pipeline_can_be_neither_made_the_default_nor_duplicated() {
  let mut editor = Editor::new();
  let draft = editor.empty("Draft only");
  editor.go(ProjectPage::Pipelines);
  editor.until("the list", |e| e.exists(&format!("pipeline-hub-row-{draft}")));
  editor.until("the panel", |e| e.exists("pipeline-hub-more"));
  editor.click("pipeline-hub-more");
  editor.until("the menu", |e| e.exists("pipeline-hub-menu-default"));
  assert!(
    editor
      .text("pipeline-hub-menu-default-detail")
      .starts_with("Publish it first")
  );
  assert!(
    editor
      .text("pipeline-hub-menu-duplicate-detail")
      .starts_with("Publish it first")
  );
  editor.click("pipeline-hub-menu-default");
  editor.click("pipeline-hub-menu-duplicate");
  assert_eq!(editor.db().default_pipeline().expect("the setting reads"), None);
  assert!(
    !matches!(
      editor.window.navigation().route_now(),
      Route::Project(_, ProjectPage::CopyPipeline(_))
    ),
    "a pipeline with no published version is not copied"
  );
}

#[test]
fn pipelines_duplicate_opens_new_pipeline_copying_the_pipeline() {
  let mut editor = Editor::new();
  let id = editor.published();
  open_menu(&mut editor, id);
  editor.click("pipeline-hub-menu-duplicate");
  editor.until("New pipeline", |e| e.exists("pipeline-name"));
  assert_eq!(
    editor.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::CopyPipeline(id))
  );
  editor.until("the pipeline chosen to copy", |e| {
    e.text("pipeline-copy").contains("Delivery")
  });
  // With Copy chosen, its row and the page's note say what it copies (`04b` `I49xek`).
  assert_eq!(editor.text("pipeline-start-1-hint"), "Published version → draft v1");
  assert!(
    editor
      .text("pipeline-nothing-saved-text")
      .starts_with("Copy lists only published pipelines.")
  );
  editor.set("pipeline-name", "Delivery copy");
  editor.click("pipeline-create");
  editor.until("the copy opened", |e| e.exists("pipeline-step-step"));
  let copy = editor.selected();
  assert_ne!(copy, id);
  let body = editor.db().draft::<Pipeline>(copy).unwrap().unwrap().body().clone();
  assert_eq!(
    body.steps()[0].name.as_str(),
    "Check",
    "draft v1 is the published version's copy"
  );
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
