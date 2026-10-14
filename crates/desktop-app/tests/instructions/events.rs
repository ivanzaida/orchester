//! Publication waits only for variables without a value at the rule's own events, never for a preview.
use orchester_core::resource::InstructionScope;

use super::harness::Instructions;

#[test]
fn instructions_a_rule_publishes_without_tasks_when_its_values_exist_at_its_event() {
  let mut screen = Instructions::new();
  let id = screen.rule(
    "Standards",
    "Working on {{task.title}}",
    InstructionScope::Project,
    false,
  );
  screen.go();
  screen.until("editor", |s| s.exists("instruction-publish"));
  assert!(!screen.exists("instruction-publication-warning"));
  screen.click("instruction-publish");
  screen.until("published", |s| s.entry(id).state.published().is_some());
}

#[test]
fn instructions_a_missing_checkpoint_is_named_without_a_preview_to_take_it_from() {
  let mut screen = Instructions::new();
  screen.rule(
    "Needs checkpoint",
    "Compare {{run.revision}}",
    InstructionScope::Project,
    false,
  );
  screen.go();
  screen.until("missing value", |s| s.exists("instruction-publication-warning"));
  let warning = screen.text("instruction-publication-warning");
  assert!(
    warning.contains("{{run.revision}} has no value at Agent start"),
    "{warning}"
  );
  assert!(warning.contains("An agent would not start with this rule"), "{warning}");
}
