//! Rejected futures retain old data; only fulfilled outcomes may advance the editor.
use crate::pipelines_support::Editor;

#[test]
fn pipelines_rejected_save_after_success_keeps_local_values_and_error() {
  let mut editor = Editor::team();
  let id = editor.empty("Repeated save");
  editor.open(id);
  editor.set("pipeline-description", "First");
  editor.click("pipeline-save");
  editor.until("first saved as v2", |e| {
    e.version(id) == 2 && !e.exists("pipeline-save")
  });
  editor.set("pipeline-description", "Local second");
  // Another editor publishes the next version behind the page: this save is made from a version no longer the latest.
  editor.publish_behind(id, "External second");
  editor.click("pipeline-save");
  editor.until("second refused", |e| e.exists("pipeline-write-error"));
  assert_eq!(editor.value("pipeline-description"), "Local second");
  assert!(editor.exists("pipeline-save"));
  assert!(editor.exists("pipeline-reload"));
  assert_eq!(editor.version(id), 3, "nothing of this page's is stored");
}

#[test]
fn pipelines_hub_refetch_failure_shows_retry_instead_of_old_success() {
  let mut editor = Editor::new();
  let id = editor.empty("Refetch");
  editor.go(orchester_app::routes::ProjectPage::Pipelines);
  editor.until("loaded hub", |e| e.exists(&format!("pipeline-hub-row-{id}")));
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
  editor.until("failed refetch", |e| e.exists("pipeline-hub-load-error"));
  editor.arm();
  assert!(editor.exists("pipeline-hub-load-error"));
  assert!(!editor.exists(&format!("pipeline-hub-row-{id}")));
  connection.pragma_update(None, "user_version", schema).unwrap();
  feed.outside.update(|counts| counts.pipelines += 1);
  editor.until("recovered hub", |e| e.exists(&format!("pipeline-hub-row-{id}")));
  assert!(!editor.exists("pipeline-hub-load-error"));
}
