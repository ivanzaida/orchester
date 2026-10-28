//! M5: a private SQLite lock retains the actual draft-save operation for inspection.
use orchester_core::resource::InstructionScope;

use super::harness::Instructions;

#[test]
fn instructions_saving_draft_keeps_publish_unavailable_and_explains_wait() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Published text", InstructionScope::Project, true);
  screen.go();
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("private db");
  db.execute_batch("BEGIN IMMEDIATE").expect("hold private writer");
  screen.set("instruction-prompt", "New draft text");
  screen.until("draft save underway", |s| {
    s.text("instruction-save-status") == "Saving draft…"
  });
  let glyph = screen
    .window
    .tree
    .get_element_by_id("instruction-publish-icon")
    .expect("Publish icon");
  assert_eq!(
    glyph.text_content(),
    Some(orchester_app::ui::icon::Icon::Lock.glyph().to_string().as_str())
  );
  screen.click("instruction-publish");
  db.execute_batch("ROLLBACK").expect("release private writer");
  screen.until("draft saved", |s| {
    s.text("instruction-save-status").contains("Based on Published v1")
  });
  let entry = screen.entry(id);
  assert!(entry.state.has_draft());
  assert_eq!(entry.state.published().expect("prior version").get(), 1);
  assert_eq!(entry.body.text().as_str(), "New draft text");
  let glyph = screen
    .window
    .tree
    .get_element_by_id("instruction-publish-icon")
    .expect("Publish ready");
  assert_eq!(
    glyph.text_content(),
    Some(orchester_app::ui::icon::Icon::Upload.glyph().to_string().as_str())
  );
}
