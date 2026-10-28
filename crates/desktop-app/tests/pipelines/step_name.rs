//! The chosen step's name, a heading edited in place (`Editable title` `BxHzj`), and the Deliver step's re-check row
//! when the pipeline has no command tool step to run again (`OY1JH` in `x5hQj`).
use crate::pipelines_support::Editor;

#[test]
fn pipelines_step_name_edits_in_place_and_esc_puts_the_name_back() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-step-step");
  assert_eq!(editor.value("step-name"), "Check");
  editor.arm();
  let bounds = |editor: &mut Editor, id: &str| {
    editor
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("laid out: {id}"))
  };
  let (input, sizer) = (bounds(&mut editor, "step-name"), bounds(&mut editor, "step-name-sizer"));
  // As wide as the name, and 2 more for the caret after its last letter.
  assert!(
    (input.x - sizer.x).abs() < 0.5 && (input.y - sizer.y).abs() < 0.5 && (input.width - sizer.width - 2.0).abs() < 0.5,
    "the input lies over the name and is as wide: {input:?} {sizer:?}"
  );
  assert!(
    editor.exists("step-name-edit"),
    "the pencil says the name can be renamed"
  );
  assert!(!editor.exists("step-name-hint"));
  editor.focus("step-name");
  editor.until("editing", |e| e.exists("step-name-hint"));
  assert_eq!(editor.text("step-name-hint"), "Enter to save · Esc to cancel");
  assert!(
    !editor.exists("step-name-edit"),
    "the pencil goes while the name is edited"
  );
  editor.set("step-name", "Checked");
  editor.key("Escape", false);
  assert_eq!(editor.value("step-name"), "Check", "Esc puts the name back");
  assert!(!editor.exists("step-name-hint"));
  assert!(editor.exists("step-name-edit"));
  editor.click("step-name-edit");
  editor.until("editing from the pencil", |e| e.exists("step-name-hint"));
  editor.set("step-name", "Checked");
  editor.key("Enter", false);
  assert!(!editor.exists("step-name-hint"), "Enter ends the editing");
  assert_eq!(editor.value("step-name"), "Checked", "and keeps the name");
  assert_eq!(editor.text("pipeline-save-status"), "Unsaved changes");
}

#[test]
fn pipelines_step_name_of_a_viewed_version_has_no_pencil() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-history");
  editor.until("versions", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  editor.until("the viewed flow", |e| e.exists("pipeline-viewing-banner"));
  editor.click("pipeline-step-step");
  assert_eq!(editor.text("step-name"), "Check");
  assert!(!editor.exists("step-name-edit"), "a viewed step's name is not edited");
}

#[test]
fn pipelines_deliver_recheck_without_a_command_tool_step_is_off_and_inert() {
  let mut editor = Editor::new();
  let id = editor.empty("No checks");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.click("pipeline-deliver-chip");
  editor.click("deliver-add");
  editor.until("the Deliver step is shown", |e| e.exists("deliver-recheck"));
  let hint = editor.text("deliver-recheck-row-hint");
  assert_eq!(
    hint, "The pipeline has no command tool step to run again on the merged result.",
    "the row says why"
  );
  let before = editor.window.texts("screen-pipelines");
  editor.click("deliver-recheck");
  editor.arm();
  assert_eq!(
    editor.window.texts("screen-pipelines"),
    before,
    "the switch is off and inert: nothing changes"
  );
}
