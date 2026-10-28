//! Regressions from the exact-head 7376ba9c review.
use orchester_core::{
  name::Name,
  resource::{Draft, Pipeline, PipelineChange, PipelineEdit, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_workspace::pipelines::{
  data::OpenedPipeline,
  history::VersionHistory,
  write::{self, Allowed},
};

use crate::pipelines_support::Editor;

#[test]
fn pipelines_restore_saved_draft_requires_explicit_replacement() {
  let mut editor = Editor::new();
  let id = editor.published();
  let latest = editor.db().latest_version::<Pipeline>(id).unwrap().unwrap();
  let body = latest
    .body()
    .clone()
    .with_description("Important saved unpublished work".into());
  editor
    .db()
    .save_draft(id, &Draft::new(body.clone(), Some(latest.number()), Timestamp::now()))
    .unwrap();
  editor.open(id);
  editor.click("pipeline-history");
  editor.until("version", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  editor.click("pipeline-restore");
  assert!(editor.exists("pipeline-restore-question"));
  assert_eq!(editor.db().draft::<Pipeline>(id).unwrap().unwrap().body(), &body);
  editor.click("pipeline-restore-keep");
  editor.click("pipeline-back-draft");
  assert_eq!(editor.value("pipeline-description"), body.description());
  editor.click("pipeline-history");
  editor.until("version again", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  editor.click("pipeline-restore");
  editor.click("pipeline-restore-confirm");
  editor.until("confirmed replacement saved", |e| {
    e.db().draft::<Pipeline>(id).unwrap().is_none()
  });
  assert_eq!(editor.db().latest_version::<Pipeline>(id).unwrap().unwrap(), latest);
}

#[test]
fn pipelines_restore_keep_preserves_unsaved_body_raw_inputs_and_errors() {
  let mut editor = Editor::new();
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
  assert!(editor.db().draft::<Pipeline>(id).unwrap().is_none());
}

#[test]
fn pipelines_failed_reload_preserves_local_editor_and_exposes_retry() {
  let mut editor = Editor::new();
  let id = editor.empty("Reload");
  editor.open(id);
  editor.set("pipeline-description", "Local unsaved");
  let external = editor
    .db()
    .draft::<Pipeline>(id)
    .unwrap()
    .unwrap()
    .body()
    .clone()
    .with_description("External".into());
  editor
    .db()
    .save_draft(id, &Draft::new(external, None, Timestamp::now()))
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
fn pipelines_publish_refuses_identical_body_recreated_on_newer_base() {
  let editor = Editor::new();
  let root = editor.window.data.dir.path().join("repo-kontur");
  let id = editor.published();
  let latest = editor.db().latest_version::<Pipeline>(id).unwrap().unwrap();
  let reviewed = latest.body().clone().with_description("Reviewed against v1".into());
  editor
    .db()
    .save_draft(
      id,
      &Draft::new(reviewed.clone(), Some(latest.number()), Timestamp::now()),
    )
    .unwrap();
  let before = OpenedPipeline::load(&editor.db(), id).unwrap();
  let external = latest.body().clone().with_description("External v2".into());
  editor
    .db()
    .save_draft(id, &Draft::new(external, Some(latest.number()), Timestamp::now()))
    .unwrap();
  let v2 = editor.db().publish_draft::<Pipeline>(id, Timestamp::now()).unwrap();
  editor
    .db()
    .save_draft(id, &Draft::new(reviewed.clone(), Some(v2.number()), Timestamp::now()))
    .unwrap();
  let error = write::publish_in(&root, &before, &Allowed::default()).unwrap_err();
  assert!(write::needs_reload(&error.message()));
  assert_eq!(editor.db().latest_version::<Pipeline>(id).unwrap().unwrap(), v2);
  assert_eq!(editor.db().draft::<Pipeline>(id).unwrap().unwrap().body(), &reviewed);
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
  editor
    .db()
    .save_draft(id, &Draft::new(v2_body.clone(), Some(v1.number()), Timestamp::now()))
    .unwrap();
  let v2 = editor.db().publish_draft::<Pipeline>(id, Timestamp::now()).unwrap();
  editor
    .db()
    .save_draft(
      id,
      &Draft::new(
        v2_body.with_description("Only introduced in v3".into()),
        Some(v2.number()),
        Timestamp::now(),
      ),
    )
    .unwrap();
  editor.db().publish_draft::<Pipeline>(id, Timestamp::now()).unwrap();
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
