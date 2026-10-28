//! ORC-30: a draft automation saved over Orchester's MCP server says so in the editor's header, until a person saves
//! the draft or publishes it; a person's Publish takes it as before.
use orchester_core::resource::InstructionScope;
use orchester_store::ProjectDb;

use super::harness::Instructions;

/// Marks the draft of rule `id` as automation's, as `save_instruction_draft` does.
fn by_automation(screen: &Instructions, id: orchester_core::id::ResourceId) {
  let saved = screen.entry(id).draft_saved_at.expect("a draft");
  ProjectDb::open(&screen.root)
    .expect("the project database")
    .mark_automation_draft(id, saved)
    .expect("marked");
}

#[test]
fn instructions_mark_a_draft_automation_wrote_until_a_person_saves_it() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Automation's words", InstructionScope::Project, false);
  by_automation(&screen, id);
  screen.go();
  screen.until("the automation mark", |s| {
    s.text("instruction-save-status") == "Draft written by automation · Not published"
  });
  screen.set("instruction-prompt", "A person's words");
  screen.until("the person's draft", |s| {
    s.text("instruction-save-status") == "Draft v1 · not published"
  });
  assert!(!screen.listed().by_automation(&screen.entry(id)));
}

#[test]
fn instructions_mark_an_automation_draft_over_a_published_version_and_a_person_publishes_it() {
  let mut screen = Instructions::new();
  let id = screen.rule("Standards", "Published words", InstructionScope::Project, true);
  let entry = screen.entry(id);
  let body = super::harness::body("Automation's words", InstructionScope::Project);
  orchester_workspace::instructions::editor::save_in(&screen.root, &entry, body).expect("automation's draft");
  by_automation(&screen, id);
  screen.go();
  screen.until("the automation mark", |s| {
    s.text("instruction-save-status") == "Draft written by automation · Published v1"
  });
  screen.click("instruction-publish");
  screen.until("published by the person", |s| {
    s.entry(id).state.published().is_some_and(|version| version.get() == 2)
  });
  screen.until("the mark is gone", |s| {
    s.text("instruction-save-status") == "Published v2"
  });
}
