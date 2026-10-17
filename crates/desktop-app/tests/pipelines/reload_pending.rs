//! Real reload success is delivered after input attempts made while the old editor is loading.
use orchester_core::{
  name::Name,
  resource::{Draft, Pipeline, PipelineEdit},
  store::ResourceStore,
  time::Timestamp,
};

use crate::pipelines_support::Editor;

#[derive(Clone, Copy)]
enum Page {
  Empty,
  Settings,
  Step,
  Remove,
  Dependencies,
}

fn starting_reload(page: Page) -> (Editor, orchester_core::id::ResourceId, Pipeline, &'static str) {
  let mut editor = Editor::new();
  let id = if matches!(page, Page::Empty) {
    editor.empty("Pending reload")
  } else {
    editor.published()
  };
  if matches!(page, Page::Dependencies) {
    let latest = editor
      .db()
      .latest_version::<Pipeline>(id)
      .expect("published fixture can be read")
      .expect("fixture has a published version");
    let mut edit = PipelineEdit::new(latest.body().clone());
    edit.add_step(Name::new("Follow-up").expect("fixture step name is valid"), None);
    editor
      .db()
      .save_draft(
        id,
        &Draft::new(
          edit.validate().expect("two-step fixture is valid"),
          Some(latest.number()),
          Timestamp::now(),
        ),
      )
      .expect("dependency fixture draft is stored");
  }
  editor.open(id);
  let field = match page {
    Page::Empty => "pipeline-description",
    Page::Settings => {
      editor.click("pipeline-settings");
      "pipeline-settings-description"
    }
    Page::Step | Page::Remove | Page::Dependencies => {
      editor.click("pipeline-step-step");
      "step-instructions"
    }
  };
  editor.set(field, "Local unsaved text");
  let current = editor
    .db()
    .draft::<Pipeline>(id)
    .expect("current fixture draft can be read");
  let latest = editor
    .db()
    .latest_version::<Pipeline>(id)
    .expect("current fixture version can be read");
  let external = current
    .as_ref()
    .map(Draft::body)
    .or_else(|| latest.as_ref().map(|v| v.body()))
    .expect("fixture supplies a draft or published pipeline")
    .clone()
    .with_description("External successful reload".into());
  editor
    .db()
    .save_draft(
      id,
      &Draft::new(external.clone(), latest.as_ref().map(|v| v.number()), Timestamp::now()),
    )
    .expect("external reload fixture edit is stored");
  editor.click("pipeline-save");
  editor.until("stale write refused", |e| e.exists("pipeline-reload"));
  match page {
    Page::Remove => editor.click("step-remove"),
    Page::Dependencies => editor.click("step-after-picker"),
    _ => {}
  }
  editor.click("pipeline-reload");
  assert!(editor.exists("pipeline-loading"), "actual read is pending delivery");
  (editor, id, external, field)
}
fn pending_reload(page: Page) {
  let (mut editor, id, external, field) = starting_reload(page);
  // Do not tick futures yet: the actual read may finish, but the live editor has not accepted it.
  editor.focus(field);
  editor.key("x", false);
  assert_eq!(
    editor.value(field),
    "Local unsaved text",
    "pending reload must refuse real typing"
  );
  attempt_controls(&mut editor, page, field);
  editor.until("successful reload accepted", |e| !e.exists("pipeline-loading"));
  // The reloaded editor keeps the panel the route shows; the summary holds the description.
  match page {
    Page::Empty => {}
    Page::Settings => editor.click("pipeline-back-steps"),
    Page::Step | Page::Remove | Page::Dependencies => editor.click("pipeline-inspector-close"),
  }
  assert_eq!(editor.value("pipeline-description"), "External successful reload");
  assert!(!editor.exists("pipeline-write-error"));
  assert_eq!(
    editor
      .db()
      .draft::<Pipeline>(id)
      .expect("accepted draft can be read")
      .expect("external draft survives accepted reload")
      .body(),
    &external
  );
  editor.focus("pipeline-description");
  editor.key("y", false);
  assert_ne!(
    editor.value("pipeline-description"),
    "External successful reload",
    "editing returns after success"
  );
}
fn attempt_controls(editor: &mut Editor, page: Page, field: &str) {
  match page {
    Page::Empty => {
      editor.click("pipeline-add-first");
      editor.click("pipeline-use-starter");
      assert!(!editor.exists("step-name"));
      assert!(!editor.exists("pipeline-step-implement"));
      assert_eq!(editor.value(field), "Local unsaved text");
    }
    Page::Settings => {
      for undo in ["pipeline-description-undo", "pipeline-settings-undo"] {
        assert!(editor.exists(undo));
        editor.click(undo);
        assert_eq!(
          editor.value(field),
          "Local unsaved text",
          "{undo} is locked during reload"
        );
      }
    }
    Page::Step => {
      editor.focus("step-name");
      let before = editor.value("step-name");
      editor.key("x", false);
      assert_eq!(editor.value("step-name"), before);
    }
    Page::Remove => {
      assert!(editor.exists("step-remove-confirm"));
      editor.click("step-remove-confirm");
      assert!(
        editor.exists("pipeline-step-step"),
        "pending removal must not alter the graph"
      );
      assert_eq!(editor.value(field), "Local unsaved text");
    }
    Page::Dependencies => {
      assert!(editor.exists("step-after-menu"), "menu was already open before reload");
      let before = editor.text("step-after-picker");
      editor.click("step-after-step-2");
      assert_eq!(
        editor.text("step-after-picker"),
        before,
        "pending reload keeps dependencies"
      );
      assert_eq!(editor.value(field), "Local unsaved text");
    }
  }
}

#[test]
fn pipelines_pending_successful_reload_locks_summary_and_empty_canvas_mutations() {
  pending_reload(Page::Empty);
}
#[test]
fn pipelines_pending_successful_reload_locks_settings_fields_and_both_undo_controls() {
  pending_reload(Page::Settings);
}
#[test]
fn pipelines_pending_successful_reload_locks_step_inspector_fields() {
  pending_reload(Page::Step);
}
#[test]
fn pipelines_pending_successful_reload_locks_preopened_remove_confirmation() {
  pending_reload(Page::Remove);
}
#[test]
fn pipelines_pending_successful_reload_locks_preopened_dependency_choices() {
  pending_reload(Page::Dependencies);
}
