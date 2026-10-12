//! Rejected futures retain old data; only fulfilled outcomes may advance the editor.
use orchester_core::{
  resource::{Draft, Pipeline},
  store::ResourceStore,
  time::Timestamp,
};

use crate::pipelines_support::Editor;

#[test]
fn pipelines_rejected_publish_after_save_keeps_review_and_error() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.set("pipeline-description", "Reviewed");
  editor.click("pipeline-save");
  editor.until("saved draft", |e| !e.exists("pipeline-save"));
  editor.click("pipeline-publish");
  assert!(editor.exists("pipeline-publish-confirm"));
  let before = editor.db().draft::<Pipeline>(id).unwrap().unwrap();
  let external = before.body().clone().with_description("External".into());
  editor
    .db()
    .save_draft(id, &Draft::new(external, before.based_on(), Timestamp::now()))
    .unwrap();
  editor.click("pipeline-publish-confirm");
  editor.until("publish refused", |e| e.exists("pipeline-write-error"));
  assert!(
    editor.exists("pipeline-publish-confirm"),
    "rejected publish must keep its review"
  );
  assert!(editor.exists("pipeline-reload"));
  assert!(!editor.exists("pipeline-publish-notice"));
  assert_eq!(
    editor
      .db()
      .latest_version::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .number()
      .get(),
    1
  );
}

#[test]
fn pipelines_rejected_save_after_success_keeps_local_values_and_error() {
  let mut editor = Editor::new();
  let id = editor.empty("Repeated save");
  editor.open(id);
  editor.set("pipeline-description", "First");
  editor.click("pipeline-save");
  editor.until("first saved", |e| !e.exists("pipeline-save"));
  editor.set("pipeline-description", "Local second");
  let before = editor.db().draft::<Pipeline>(id).unwrap().unwrap();
  let external = before.body().clone().with_description("External second".into());
  editor
    .db()
    .save_draft(id, &Draft::new(external, before.based_on(), Timestamp::now()))
    .unwrap();
  editor.click("pipeline-save");
  editor.until("second refused", |e| e.exists("pipeline-write-error"));
  assert_eq!(editor.value("pipeline-description"), "Local second");
  assert!(editor.exists("pipeline-save"));
  assert!(editor.exists("pipeline-reload"));
}

#[test]
fn pipelines_library_refetch_failure_shows_retry_instead_of_old_success() {
  let mut editor = Editor::new();
  let id = editor.empty("Refetch");
  editor.go(orchester_app::routes::ProjectPage::Pipelines);
  editor.until("loaded library", |e| e.exists(&format!("pipeline-row-{id}")));
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
  let feed = editor.window.projects().feed(crate::support::project_id());
  feed.outside.update(|counts| counts.pipelines += 1);
  editor.until("failed refetch", |e| e.exists("pipelines-error"));
  assert!(!editor.exists(&format!("pipeline-row-{id}")));
  connection.pragma_update(None, "user_version", schema).unwrap();
  feed.outside.update(|counts| counts.pipelines += 1);
  editor.until("recovered library", |e| e.exists(&format!("pipeline-row-{id}")));
  assert!(!editor.exists("pipelines-error"));
}

#[test]
fn pipelines_restore_waits_for_pending_save_before_replacing() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  let path = editor
    .window
    .data
    .dir
    .path()
    .join("repo-kontur/.orchester/orchester.db");
  let mut connection = rusqlite::Connection::open(path).unwrap();
  let lock = connection
    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
    .unwrap();
  editor.set("pipeline-description", "Pending work");
  editor.click("pipeline-save");
  editor.click("pipeline-history");
  editor.until("history during write", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  editor.click("pipeline-restore");
  assert!(
    !editor.exists("pipeline-restore-question"),
    "restore is unavailable while saving"
  );
  lock.commit().unwrap();
  editor.until("pending save completed", |e| {
    e.db().draft::<Pipeline>(id).unwrap().is_some()
  });
  editor.arm();
  editor.click("pipeline-restore");
  assert!(editor.exists("pipeline-restore-question"));
  editor.click("pipeline-restore-keep");
  editor.click("pipeline-back-draft");
  assert_eq!(editor.value("pipeline-description"), "Pending work");
}
