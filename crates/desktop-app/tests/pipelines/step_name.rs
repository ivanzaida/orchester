//! The chosen step's name, a heading edited in place (`Editable title` `BxHzj`), and the Deliver step's re-check row
//! when the pipeline has no command tool step to run again (`OY1JH` in `x5hQj`).
use crate::pipelines_support::Editor;

#[test]
fn pipelines_step_name_of_a_viewed_version_has_no_pencil() {
  let mut editor = Editor::team();
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
  let mut editor = Editor::team();
  let id = editor.empty("No checks");
  editor.open(id);
  // The fixture's one step runs a command tool: without it, an added step runs nothing to check again.
  editor.remove_only_step();
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
