//! Regressions from the exact-head 7376ba9c review.
use orchester_core::{
  name::Name,
  resource::{Pipeline, PipelineChange, PipelineEdit, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_workspace::pipelines::history::VersionHistory;

use crate::pipelines_support::Editor;

#[test]
fn pipelines_restore_keep_preserves_unsaved_body_raw_inputs_and_errors() {
  let mut editor = Editor::team();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-step-step");
  editor.set("step-budget", "invalid budget");
  editor.click("step-instructions-row");
  editor.set("step-instructions", "Local work");
  editor.click("pipeline-history");
  editor.until("version", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  editor.click("pipeline-restore");
  assert!(editor.exists("pipeline-restore-question"));
  editor.click("pipeline-restore-keep");
  editor.click("pipeline-back-draft");
  editor.click("pipeline-step-step");
  assert_eq!(editor.value("step-budget"), "invalid budget");
  editor.click("step-instructions-row");
  assert_eq!(editor.value("step-instructions"), "Local work");
  assert!(editor.exists("pipeline-field-error"));
  assert_eq!(editor.version(id), 1, "an invalid edit is not saved");
}

#[test]
fn pipelines_failed_reload_preserves_local_editor_and_exposes_retry() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.set("pipeline-description", "Local unsaved");
  // Another editor saves the next version behind the page (a pipeline holds no draft, D-032).
  let latest = editor.db().latest_version::<Pipeline>(id).unwrap().unwrap();
  let external = latest.body().clone().with_description("External".into());
  editor
    .db()
    .publish_pipeline_edit(id, &external, latest.number(), None, Timestamp::now())
    .unwrap();
  editor.click("pipeline-save");
  editor.until("conflict", |e| e.exists("pipeline-reload"));
  let path = editor
    .window
    .data
    .dir
    .path()
    .join("repo-kontur/.orchester/orchester.db");
  let connection = rusqlite::Connection::open(path).unwrap();
  let schema: u32 = connection
    .pragma_query_value(None, "user_version", |r| r.get(0))
    .unwrap();
  connection.pragma_update(None, "user_version", 99999).unwrap();
  editor.click("pipeline-reload");
  editor.until("reload rejected visibly", |e| e.exists("pipeline-load-error"));
  assert_eq!(editor.value("pipeline-description"), "Local unsaved");
  assert!(editor.exists("pipeline-write-error"));
  connection.pragma_update(None, "user_version", schema).unwrap();
  editor.click("pipeline-reload");
  editor.until("fresh reload", |e| e.value("pipeline-description") == "External");
  assert!(!editor.exists("pipeline-load-error"));
}

#[test]
fn pipelines_history_keeps_immutable_changes_separate_from_comparison() {
  let mut editor = Editor::new();
  let id = editor.published();
  let v1 = editor.db().latest_version::<Pipeline>(id).unwrap().unwrap();
  let mut changed = PipelineEdit::new(v1.body().clone());
  let key = changed.steps()[0].key.clone();
  changed.update(&key, |s| s.name = Name::new("Revised").unwrap());
  let v2_body = changed.validate().unwrap();
  let v2 = editor
    .db()
    .publish_pipeline_edit(id, &v2_body, v1.number(), None, Timestamp::now())
    .unwrap();
  editor
    .db()
    .publish_pipeline_edit(
      id,
      &v2_body.with_description("Only introduced in v3".into()),
      v2.number(),
      None,
      Timestamp::now(),
    )
    .unwrap();
  let history = VersionHistory::load(&editor.db(), id).unwrap();
  let v2 = history.version(VersionNumber::FIRST.next().unwrap()).unwrap();
  assert!(
    !v2
      .changes
      .as_ref()
      .unwrap()
      .changes
      .contains(&PipelineChange::Description)
  );
  editor.open(id);
  editor.click("pipeline-history");
  editor.until("history v2", |e| e.exists("pipeline-version-2"));
  editor.click("pipeline-version-2");
  editor.until("v2's changes", |e| e.exists("pipeline-version-changes"));
  assert!(!editor.text("screen-pipelines").contains("description edited"));
  editor.click("pipeline-compare");
  assert!(editor.text("screen-pipelines").contains("description edited"));
  assert_eq!(editor.text("pipeline-version-changes-title"), "Changes in v2");
  assert_eq!(
    editor.text("pipeline-comparison-changes-title"),
    "Compared with published v3"
  );
  // Leaving the version for one of its steps and going back to Versions used to keep comparison hidden.
  editor.click("pipeline-step-step");
  editor.click("pipeline-back-versions");
  editor.until("versions remounted", |e| e.exists("pipeline-version-2"));
  assert!(!editor.exists("pipeline-comparison-changes"));
  assert!(!editor.text("screen-pipelines").contains("description edited"));
}
