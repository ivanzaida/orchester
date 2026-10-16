//! A saved draft reads the instructions again once, as the change it records, and shows what was saved.

use orchester_core::resource::InstructionScope;

use super::{harness::Instructions, support::reads::Reads};

#[test]
fn instructions_save_reads_the_instructions_once() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Published text", InstructionScope::Project, true);
  screen.go();
  screen.pump();
  let reads = Reads::count();
  screen.set("instruction-prompt", "New draft text");
  screen.until("draft saved", |s| {
    s.text("instruction-save-status").contains("Draft saved")
  });
  screen.pump();
  let saved = reads.take();
  assert_eq!(saved.get("instructions"), Some(&1), "{saved:?}");
  assert_eq!(screen.entry(id).body.text().as_str(), "New draft text");
  assert!(
    screen.text("screen-instructions").contains("Draft saved"),
    "the screen shows the saved draft"
  );
}
