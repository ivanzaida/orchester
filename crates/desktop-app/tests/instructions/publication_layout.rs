//! M2: publication failure uses GxmNs's inset notice and retains the saved draft.
use orchester_core::resource::InstructionScope;
use orchester_workspace::instructions::editor;

use super::harness::{Instructions, body};

fn bounds(screen: &mut Instructions, id: &str) -> lurq::core::ElementRect {
  screen
    .window
    .tree
    .get_element_by_id_mut(id)
    .expect("element")
    .bounds()
    .expect("laid out")
}

fn close(actual: f32, expected: f32) {
  assert!((actual - expected).abs() < 1.0, "expected {expected}, got {actual}");
}

#[test]
fn instructions_publication_failure_is_inset_with_retry_below_text_and_retains_draft() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Published text", InstructionScope::Project, true);
  editor::save_in(
    &screen.root,
    &screen.entry(id),
    body("Saved draft text", InstructionScope::Project),
  )
  .expect("saved private draft");
  let before = screen.entry(id);
  screen.go();
  screen.until("saved draft", |s| {
    s.text("instruction-save-status").contains("Draft saved")
  });
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("private db");
  db.execute_batch(
    "CREATE TRIGGER private_publish_failure BEFORE INSERT ON resource_versions BEGIN SELECT RAISE(FAIL, 'private publication failure'); END;"
  ).expect("private failure trigger");
  screen.click("instruction-publish");
  screen.until("publication failure", |s| s.exists("instruction-write-error"));
  assert_eq!(
    screen.entry(id),
    before,
    "failed publication keeps draft and published version"
  );
  assert_eq!(screen.text("instruction-prompt"), "Saved draft text");
  assert!(
    !screen.exists("instruction-reload"),
    "publication error has only its retry action"
  );
  let header = bounds(&mut screen, "instruction-editor-header");
  let notice = bounds(&mut screen, "instruction-write-error");
  let title = bounds(&mut screen, "instruction-write-error-title");
  let explanation = bounds(&mut screen, "instruction-write-error-body");
  let glyph = bounds(&mut screen, "instruction-write-error-icon");
  let retry = bounds(&mut screen, "instruction-retry");
  close(notice.x - header.x, 20.0);
  close(header.x + header.width - notice.x - notice.width, 20.0);
  close(notice.y - header.y - header.height, 12.0);
  close(glyph.x - notice.x, 14.0);
  close(glyph.y - notice.y, 12.0);
  close(title.x - glyph.x - glyph.width, 10.0);
  close(explanation.x, title.x);
  close(retry.x, title.x);
  close(retry.y - explanation.y - explanation.height, 10.0);
  close(notice.y + notice.height - retry.y - retry.height, 12.0);
  db.execute_batch("DROP TRIGGER private_publish_failure;")
    .expect("release private failure");
  screen.click("instruction-retry");
  screen.until("publication retried", |s| {
    !s.exists("instruction-write-error") && s.entry(id).state.published().is_some_and(|v| v.get() == 2)
  });
  assert!(!screen.entry(id).state.has_draft());
  assert_eq!(screen.entry(id).body.text().as_str(), "Saved draft text");
}

#[test]
fn instructions_publication_conflict_keeps_reload_available_and_preserves_outside_draft() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Opened draft", InstructionScope::Project, false);
  screen.go();
  screen.until("ready to publish", |s| s.exists("instruction-variable-insert"));
  let outside = editor::save_in(
    &screen.root,
    &screen.entry(id),
    body("Outside draft", InstructionScope::Project),
  )
  .expect("outside draft");
  screen.click("instruction-publish");
  screen.until("publication conflict", |s| s.exists("instruction-write-error"));
  assert!(
    screen.exists("instruction-reload"),
    "conflicting publication still offers recovery"
  );
  assert_eq!(screen.entry(id), outside);
  screen.click("instruction-reload");
  screen.until("outside draft loaded", |s| {
    !s.exists("instruction-write-error") && s.text("instruction-prompt") == "Outside draft"
  });
  assert_eq!(screen.entry(id), outside, "reload never publishes the outside draft");
}

#[test]
fn instructions_edit_after_failed_publication_keeps_reload_available() {
  let mut screen = Instructions::new();
  screen.rule("Empty", "", InstructionScope::Project, false);
  screen.go();
  screen.click("instruction-publish");
  screen.until("publication refused", |s| s.exists("instruction-write-error"));
  assert!(!screen.exists("instruction-reload"));
  screen.set("instruction-prompt", "New candidate");
  assert!(
    screen.exists("instruction-reload"),
    "changed input can still recover from the rejected write"
  );
}
