//! The project's default pipeline (ORC-106): made from a published row's `…` menu, shown with a *Default* chip,
//! stored with the project and chosen at first in New task; and *Duplicate*, which opens New pipeline copying it.
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{resource::Pipeline, store::ResourceStore};

use crate::{pipelines_support::Editor, support::project_id};

#[test]
fn pipelines_default_is_made_from_the_row_menu_and_new_task_chooses_it() {
  let mut editor = Editor::new();
  let id = editor.published();
  let draft = editor.empty("Draft only");
  editor.go(ProjectPage::Pipelines);
  editor.until("the list", |e| e.exists(&format!("pipeline-row-{id}")));
  assert!(
    !editor.exists(&format!("pipeline-row-menu-{draft}")),
    "a draft-only row has no menu"
  );
  assert!(editor.text("pipelines-count").contains("2 pipelines · 1 published"));
  editor.click(&format!("pipeline-row-menu-{id}"));
  assert!(editor.exists(&format!("pipeline-row-open-{id}")));
  assert!(editor.exists(&format!("pipeline-row-duplicate-{id}")));
  editor.click(&format!("pipeline-row-default-{id}"));
  editor.until("the Default chip", |e| e.exists(&format!("pipeline-row-{id}-default")));
  assert_eq!(editor.db().default_pipeline().expect("the setting reads"), Some(id));
  editor.click(&format!("pipeline-row-menu-{id}"));
  assert!(
    !editor.exists(&format!("pipeline-row-default-{id}")),
    "the default is not offered again"
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
fn pipelines_duplicate_opens_new_pipeline_copying_the_row() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.go(ProjectPage::Pipelines);
  editor.until("the list", |e| e.exists(&format!("pipeline-row-{id}")));
  editor.click(&format!("pipeline-row-menu-{id}"));
  editor.click(&format!("pipeline-row-duplicate-{id}"));
  editor.until("New pipeline", |e| e.exists("pipeline-name"));
  assert_eq!(
    editor.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::CopyPipeline(id))
  );
  editor.until("the row's pipeline chosen to copy", |e| {
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
