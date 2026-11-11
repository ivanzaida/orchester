//! Real reload success is delivered after input attempts made while the old editor is loading. Another editor's next
//! version makes the page's save stale (a pipeline holds no draft, D-032).
use orchester_core::{
  name::Name,
  resource::{Pipeline, PipelineEdit},
  store::ResourceStore,
  time::Timestamp,
};

use crate::pipelines_support::Editor;

#[derive(Clone, Copy)]
enum Page {
  Settings,
  Step,
  Remove,
  Dependencies,
}

fn starting_reload(page: Page) -> (Editor, orchester_core::id::ResourceId, Pipeline, &'static str) {
  let mut editor = Editor::team();
  let id = editor.published();
  if matches!(page, Page::Dependencies) {
    let latest = editor
      .db()
      .latest_version::<Pipeline>(id)
      .expect("published fixture can be read")
      .expect("fixture has a published version");
    let mut edit = PipelineEdit::new(latest.body().clone());
    // The second step runs what the first does: a stored pipeline's steps all run something.
    let kind = latest.body().steps()[0].kind.clone();
    let follow = edit.add_step(Name::new("Follow-up").expect("fixture step name is valid"), None);
    edit.update(&follow, |step| step.kind = kind);
    editor
      .db()
      .publish_pipeline_edit(
        id,
        &edit.validate().expect("two-step fixture is valid"),
        latest.number(),
        None,
        Timestamp::now(),
      )
      .expect("dependency fixture version is stored");
  }
  editor.open(id);
  let field = match page {
    Page::Settings => {
      editor.click("pipeline-settings");
      "pipeline-settings-description"
    }
    Page::Step | Page::Remove | Page::Dependencies => {
      editor.click("pipeline-step-step");
      "step-name"
    }
  };
  editor.set(field, "Local unsaved text");
  editor.publish_behind(id, "External successful reload");
  let external = editor.stored(id);
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
  // The reloaded page keeps what the route shows; the summary holds the description.
  match page {
    Page::Settings => editor.click("pipeline-tab-steps"),
    Page::Step | Page::Remove | Page::Dependencies => editor.click("pipeline-start"),
  }
  assert_eq!(editor.value("pipeline-description"), "External successful reload");
  assert!(!editor.exists("pipeline-write-error"));
  assert_eq!(
    editor.stored(id),
    external,
    "their version survives the accepted reload"
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
    Page::Settings => {
      assert!(editor.exists("pipeline-settings-undo"));
      editor.click("pipeline-settings-undo");
      assert_eq!(
        editor.value(field),
        "Local unsaved text",
        "Undo change is locked during reload"
      );
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
