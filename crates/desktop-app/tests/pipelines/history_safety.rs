//! Regressions from the exact-head 7376ba9c review.
use orchester_core::{
  name::Name,
  resource::{PipelineChange, PipelineEdit, VersionNumber},
};

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
  let workspace = editor.window.launch.workspace().expect("the session").clone();
  let read = workspace.team_read(editor.repo()).config().cloned();
  let read = read.expect("the team's configuration is read");
  let schema: u32 = connection
    .pragma_query_value(None, "user_version", |r| r.get(0))
    .unwrap();
  connection.pragma_update(None, "user_version", 99999).unwrap();
  editor.click("pipeline-reload");
  editor.until("reload rejected visibly", |e| e.exists("pipeline-load-error"));
  assert_eq!(editor.value("pipeline-description"), "Local unsaved");
  assert!(editor.exists("pipeline-write-error"));
  connection.pragma_update(None, "user_version", schema).unwrap();
  // A project whose database does not read is not followed, and its team's configuration leaves memory; the stream
  // reads it again once the database reads. No stream follows the stand-in here, so the test keeps what it read.
  workspace.keep_team_config_for_test(editor.repo(), (*read).clone());
  editor.click("pipeline-reload");
  editor.until("fresh reload", |e| e.value("pipeline-description") == "External");
  assert!(!editor.exists("pipeline-load-error"));
}

#[test]
fn pipelines_history_keeps_each_versions_own_changes() {
  let mut editor = Editor::team();
  let id = editor.published();
  let mut changed = PipelineEdit::new(editor.stored(id));
  let key = changed.steps()[0].key.clone();
  changed.update(&key, |s| s.name = Name::new("Revised").unwrap());
  let v2_body = changed.validate().unwrap();
  editor.publish_next(id, v2_body.clone());
  editor.publish_next(id, v2_body.with_description("Only introduced in v3".into()));
  let history = editor.team_history(id);
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
