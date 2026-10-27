use orchester_core::resource::InstructionScope;

use super::{harness::Instructions, preview::seed_preview};

#[test]
fn instructions_variable_picker_replaces_selection_and_autosaves_exact_raw_bytes() {
  let mut screen = Instructions::new();
  seed_preview(&screen);
  let id = screen.rule("Standards", "left middle right", InstructionScope::Project, true);
  screen.go();
  screen.click("instruction-variables");
  screen.until("catalog", |screen| screen.exists("instruction-variable-insert"));
  screen.focus_prompt();
  screen.key("Home", "Home", false, true);
  for _ in 0..5 {
    screen.key("ArrowRight", "ArrowRight", false, false);
  }
  for _ in 0..6 {
    screen.key("ArrowRight", "ArrowRight", true, false);
  }
  screen.click("instruction-variable-insert");
  screen.until("inserted selection", |screen| {
    screen.text("instruction-prompt") == "left {{task.title}} right"
  });
  assert_eq!(
    screen.window.tree.focused_element().and_then(|element| element.id()),
    Some("instruction-prompt")
  );
  screen.key("z", "KeyZ", false, true);
  assert_eq!(screen.text("instruction-prompt"), "left middle right");
  screen.key("z", "KeyZ", true, true);
  screen.until("inserted source saved", |screen| {
    screen.entry(id).body.text().as_str() == "left {{task.title}} right"
      && screen.text("instruction-save-status").contains("Based on Published v1")
  });
}
