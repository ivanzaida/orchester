//! A retained publication receipt must never cover a new in-place decision.
use orchester_core::{id::ResourceId, resource::Pipeline, store::ResourceStore};

use crate::pipelines_support::Editor;

fn published_notice() -> (Editor, ResourceId) {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  save_description(&mut editor, "Published second version");
  editor.click("pipeline-publish");
  editor.until("ordinary publish review", |e| e.exists("pipeline-publish-confirm"));
  assert!(
    !editor.gated(),
    "description-only publication has no protected decision"
  );
  editor.click("pipeline-publish-confirm");
  editor.until("actual publication receipt", |e| e.exists("pipeline-publish-notice"));
  assert_eq!(latest(&editor, id).description(), "Published second version");
  (editor, id)
}

fn latest(editor: &Editor, id: ResourceId) -> Pipeline {
  editor
    .db()
    .latest_version::<Pipeline>(id)
    .expect("private fixture database reads published version")
    .expect("fixture has a published version")
    .body()
    .clone()
}

fn save_description(editor: &mut Editor, text: &str) {
  editor.set("pipeline-description", text);
  editor.click("pipeline-save");
  editor.until("description actually saved", |e| !e.exists("pipeline-save"));
}

fn unobscured(editor: &mut Editor, buttons: &[&str]) {
  assert!(
    !editor.exists("pipeline-publish-notice"),
    "retained receipt must not cover decisions"
  );
  for id in buttons {
    let rect = editor
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.bounds())
      .unwrap_or_else(|| panic!("laid out decision: {id}"));
    assert!(rect.width > 0.0 && rect.height > 0.0, "{id}: {rect:?}");
    assert!(
      rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.width <= 1440.0 && rect.y + rect.height <= 900.0,
      "decision outside viewport: {id}: {rect:?}"
    );
  }
}

fn pointer_click(editor: &mut Editor, id: &str) {
  let rect = editor
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|e| e.bounds())
    .expect("ordinary Keep button laid out");
  let (x, y) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
  editor.window.tree.mouse_move(x, y);
  editor
    .window
    .tree
    .mouse_down(x, y, lurq::app::events::MouseButton::Left);
  editor.window.tree.mouse_up(x, y, lurq::app::events::MouseButton::Left);
  editor.window.pass();
  editor.window.pass();
}

#[test]
fn pipelines_publication_notice_yields_to_protected_save_without_answering() {
  let (mut editor, id) = published_notice();
  let published = latest(&editor, id);
  editor.click("pipeline-step-step");
  editor.click("step-gate");
  editor.click("pipeline-save");
  editor.until("protected Save card", |e| e.exists("pipeline-save-question-confirm"));
  assert!(editor.gated(), "person-only Save protection remains armed");
  unobscured(
    &mut editor,
    &["pipeline-save-question-keep", "pipeline-save-question-confirm"],
  );
  assert_eq!(latest(&editor, id), published);
  assert!(
    editor
      .db()
      .draft::<Pipeline>(id)
      .expect("read unchanged draft")
      .is_none()
  );
}

#[test]
fn pipelines_publication_notice_yields_to_next_publish_review_and_returns_after_cancel() {
  let (mut editor, id) = published_notice();
  save_description(&mut editor, "Unpublished third version");
  let saved = editor
    .db()
    .draft::<Pipeline>(id)
    .expect("read saved draft")
    .expect("saved draft exists");
  editor.click("pipeline-publish");
  editor.until("next publication review", |e| e.exists("pipeline-publish-confirm"));
  unobscured(&mut editor, &["pipeline-publish-cancel", "pipeline-publish-confirm"]);
  pointer_click(&mut editor, "pipeline-publish-cancel");
  assert!(
    editor.exists("pipeline-publish-notice"),
    "receipt is retained, not dismissed"
  );
  assert_eq!(
    editor.db().draft::<Pipeline>(id).expect("read retained draft"),
    Some(saved)
  );
  assert_eq!(latest(&editor, id).description(), "Published second version");
}

#[test]
fn pipelines_publication_notice_yields_to_leave_keep_preserving_invalid_edits() {
  let (mut editor, id) = published_notice();
  editor.click("pipeline-step-step");
  editor.set("step-budget", "invalid pending input");
  editor.click("pipeline-parent");
  editor.until("Leave card", |e| e.exists("pipeline-leave-keep"));
  unobscured(&mut editor, &["pipeline-leave-keep", "pipeline-leave-confirm"]);
  pointer_click(&mut editor, "pipeline-leave-keep");
  assert!(editor.exists("pipeline-publish-notice"));
  assert_eq!(editor.selected(), id);
  assert_eq!(editor.value("step-budget"), "invalid pending input");
  assert!(editor.db().draft::<Pipeline>(id).expect("read unsaved draft").is_none());
}

#[test]
fn pipelines_publication_notice_yields_to_restore_keep_preserving_saved_draft() {
  let (mut editor, id) = published_notice();
  save_description(&mut editor, "Retained unpublished third version");
  let saved = editor
    .db()
    .draft::<Pipeline>(id)
    .expect("read saved draft")
    .expect("saved draft exists");
  editor.click("pipeline-history");
  editor.until("prior version", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  editor.click("pipeline-restore");
  unobscured(&mut editor, &["pipeline-restore-keep", "pipeline-restore-confirm"]);
  pointer_click(&mut editor, "pipeline-restore-keep");
  assert!(editor.exists("pipeline-publish-notice"));
  assert_eq!(
    editor.db().draft::<Pipeline>(id).expect("read retained draft"),
    Some(saved)
  );
  assert_eq!(latest(&editor, id).description(), "Published second version");
}
