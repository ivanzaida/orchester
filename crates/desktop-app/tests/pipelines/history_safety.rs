//! Regressions from the exact-head 7376ba9c review.
use orchester_core::{
  name::Name,
  resource::{Pipeline, PipelineChange, PipelineEdit, VersionNumber},
  store::ResourceStore,
};
use orchester_workspace::pipelines::history::VersionHistory;

use crate::pipelines_support::Editor;

#[test]
fn pipelines_failed_reload_preserves_local_editor_and_exposes_retry() {
  let mut editor = Editor::team();
  let id = editor.published();
  editor.open(id);
  editor.set("pipeline-description", "Local unsaved");
  // Another editor saves the next version behind the page (a pipeline holds no draft, D-032).
  editor.publish_behind(id, "External");
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
fn pipelines_history_keeps_each_versions_own_changes() {
  let mut editor = Editor::team();
  let id = editor.published();
  let v1 = editor.db().latest_version::<Pipeline>(id).unwrap().unwrap();
  let mut changed = PipelineEdit::new(v1.body().clone());
  let key = changed.steps()[0].key.clone();
  changed.update(&key, |s| s.name = Name::new("Revised").unwrap());
  let v2_body = changed.validate().unwrap();
  editor.publish_next(id, v2_body.clone());
  editor.publish_next(id, v2_body.with_description("Only introduced in v3".into()));
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
  assert_eq!(editor.text("pipeline-version-changes-title"), "Changes in v2");
}
