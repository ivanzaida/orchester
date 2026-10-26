//! A saved draft reads the instructions again once, as the change it records, and shows what was saved.

use orchester_core::resource::InstructionScope;
use orchester_workspace::instructions::editor;

use super::{
  harness::{Instructions, body},
  support::reads::Reads,
};

#[test]
fn instructions_save_reads_the_instructions_once() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Published text", InstructionScope::Project, true);
  screen.go();
  screen.pump();
  let reads = Reads::count();
  screen.set("instruction-prompt", "New draft text");
  screen.until("draft saved", |s| {
    s.text("instruction-save-status").contains("Based on Published v1")
  });
  screen.pump();
  let saved = reads.take();
  assert_eq!(saved.get("instructions"), Some(&1), "{saved:?}");
  assert_eq!(screen.entry(id).body.text().as_str(), "New draft text");
  assert!(
    screen.text("instruction-save-status").contains("Draft v2 · saved"),
    "the screen shows the saved draft"
  );
}

#[test]
fn instructions_reload_reads_the_list_again_too() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Opened draft", InstructionScope::Project, false);
  screen.go();
  screen.until("ready to publish", |s| s.exists("instruction-variable-insert"));
  // Written by another program: nothing tells the window.
  editor::save_in(
    &screen.root,
    &screen.entry(id),
    body("Outside draft", InstructionScope::Project),
  )
  .expect("outside draft");
  screen.click("instruction-publish");
  screen.until("publication conflict", |s| s.exists("instruction-write-error"));
  screen.pump();
  let reads = Reads::count();
  screen.click("instruction-reload");
  screen.until("outside draft loaded", |s| {
    !s.exists("instruction-write-error") && s.text("instruction-prompt") == "Outside draft"
  });
  screen.pump();
  let reloaded = reads.take();
  assert_eq!(reloaded.get("instructions"), Some(&1), "{reloaded:?}");
  assert!(
    !screen.exists("instruction-reload"),
    "the list read again holds what was reloaded"
  );
}
