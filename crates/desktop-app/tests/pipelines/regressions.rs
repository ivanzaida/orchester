//! Review regressions: input identity and leaving with pending edits. A pipeline holds no draft (D-032): what the page
//! saves is its next version.
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::resource::StepKind;

use crate::pipelines_support::Editor;

/// Text typed with the keyboard in each field, the caret moved back within it, is saved exactly as typed. Nothing saves
/// on its own (PV2-32), so typing is never interrupted by a save; pressing *Save* takes the keyboard's focus.
#[test]
fn pipelines_keyboard_typed_text_in_each_field_is_saved_as_typed() {
  let mut editor = Editor::team();
  let id = editor.empty("Typing");
  editor.open(id);
  type_and_save(&mut editor, "pipeline-description", "a");
  editor.click("pipeline-settings");
  type_and_save(&mut editor, "pipeline-settings-description", "d");
  editor.click("pipeline-tab-steps");
  // The fixture's one step, which runs a tool: a step added here would run nothing, which saving refuses.
  editor.click("pipeline-step-step");
  editor.click("step-instructions-row");
  type_and_save(&mut editor, "step-instructions", "g");
}

fn type_and_save(editor: &mut Editor, field: &str, first: &str) {
  editor.set(field, "");
  editor.focus(field);
  for key in [first, "b", "c", "ArrowLeft", "y"] {
    editor.key(key, false);
  }
  let typed = format!("{first}byc");
  assert_eq!(editor.value(field), typed, "the caret moved back within the text");
  assert!(editor.exists("pipeline-save"), "each field must cause a new save");
  editor.save();
  editor.until("saved", |e| {
    !e.exists("pipeline-save") && e.text("pipeline-save-status").starts_with("Saved as")
  });
  let saved = editor.stored(editor.selected());
  let saved_text = if field == "step-instructions" {
    saved.steps()[0].instructions.as_str()
  } else {
    saved.description()
  };
  assert_eq!(saved_text, typed, "the typed value reached the database");
  assert_eq!(editor.value(field), typed, "the field shows what was saved");
}

#[test]
fn pipelines_leaving_with_unsaved_edits_requires_a_decision_and_keep_preserves_edits() {
  let mut editor = Editor::team();
  let id = editor.empty("Pending");
  editor.go(ProjectPage::Pipelines);
  editor.open(id);
  editor.set("pipeline-description", "Pending description");
  editor.click("pipeline-parent");
  assert_eq!(editor.selected(), id);
  assert!(editor.exists("pipeline-leave-question"));
  assert_eq!(editor.value("pipeline-description"), "Pending description");
  assert_eq!(editor.stored(id).description(), "");
  editor.click("pipeline-leave-keep");
  assert!(!editor.exists("pipeline-leave-question"));
  assert_eq!(
    editor.value("pipeline-description"),
    "Pending description",
    "the edit is kept"
  );
  editor.click("pipeline-save");
  editor.until("kept work saved", |e| {
    e.stored(id).description() == "Pending description"
  });
  // The team server's answer is stored before the page settles: leaving waits until it says the version is saved.
  editor.until("the page says saved", |e| e.exists("pipeline-header-history"));
  editor.click("pipeline-parent");
  editor.until("the list", |e| e.exists(&format!("pipeline-row-{id}")));
  editor.open(id);
  assert_eq!(editor.value("pipeline-description"), "Pending description");
}

#[test]
fn pipelines_sidebar_history_and_tabs_preserve_unconfirmed_and_invalid_edits() {
  let mut editor = Editor::team();
  let id = editor.published();
  editor.go(ProjectPage::Pipelines);
  editor.open(id);
  editor.click("pipeline-step-step");
  editor.click("step-gate");
  editor.click("nav-tasks");
  assert_eq!(editor.selected(), id);
  assert!(editor.exists("pipeline-leave-question"));
  editor.click("pipeline-leave-keep");
  // Back first closes the step's panel, a place of the editor's own, and keeps its edits unasked.
  editor.window.navigation().back();
  editor.window.pass();
  assert_eq!(editor.selected(), id);
  assert!(!editor.exists("pipeline-leave-question"));
  assert!(editor.exists("pipeline-summary-name"));
  editor.window.navigation().back();
  editor.window.pass();
  assert_eq!(editor.selected(), id);
  assert!(editor.exists("pipeline-leave-question"));
  editor.click("pipeline-leave-keep");
  editor.window.navigation().forward();
  editor.window.pass();
  assert!(editor.exists("step-budget"), "Forward reopens the step with its edits");
  editor.window.navigation().open_tab();
  editor.window.pass();
  assert_eq!(editor.window.navigation().tabs_now().tabs().len(), 1);
  assert!(editor.exists("pipeline-leave-question"));
  editor.click("pipeline-leave-keep");
  editor.click("pipeline-save");
  editor.until("protected save", |e| e.exists("pipeline-save-question-confirm"));
  assert!(editor.gated(), "navigation must never authorize the weaker edit");
  assert_eq!(editor.version(id), 1, "nothing is saved while the person decides");
  editor.click("pipeline-save-question-keep");
  editor.set("step-budget", "invalid");
  editor.window.navigation().close_active();
  editor.window.pass();
  assert!(editor.exists("pipeline-leave-question"));
  assert_eq!(editor.value("step-budget"), "invalid");
  editor.click("pipeline-leave-keep");
  editor.click("nav-tasks");
  editor.click("pipeline-leave-confirm");
  editor.window.until("departed", |w| {
    matches!(w.navigation().route_now(), Route::Project(_, ProjectPage::Tasks(_)))
  });
  assert_eq!(editor.version(id), 1, "explicit discard never saves");
  editor.open(id);
  editor.click("pipeline-step-step");
  assert_eq!(editor.value("step-budget"), "No limit");
  editor.click("pipeline-parent");
  editor.until("clean departure", |e| e.exists(&format!("pipeline-row-{id}")));
}

#[test]
fn pipelines_executor_reset_keeps_displayed_and_saved_model_in_sync() {
  let mut editor = Editor::team();
  editor.profile();
  editor.published(); // Supplies the Checks tool.
  let id = editor.empty("Model transitions");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.choose("step-executor", "Reviewer");
  editor.choose("step-model", "test-model");
  editor.save();
  // The added step is the agent step; the fixture's first runs a tool.
  editor.until("model override saved", |e| {
    e.stored(id).steps().iter().any(|step| {
      matches!(&step.kind, StepKind::Agent { model: Some(model), .. }
        if model.as_str() == "test-model")
    })
  });
  editor.choose("step-executor", "Choose a profile");
  assert!(
    editor.text("step-model").contains("test-model"),
    "agent changes keep the override"
  );
  editor.choose("step-executor", "Reviewer");
  editor.choose("step-executor", "Checks");
  assert!(!editor.exists("step-model"));
  editor.choose("step-executor", "Reviewer");
  // Taken from the profile, the box shows the profile's model id (`DuMJh` in `IsU5u`).
  assert_eq!(editor.text("step-model-label"), "test-model");
  editor.save();
  editor.until("inherited model saved", |e| {
    e.stored(id).steps().iter().any(|step| {
      matches!(
        step.kind,
        StepKind::Agent {
          model: None,
          profile: Some(_),
          ..
        }
      )
    })
  });
  assert_eq!(editor.text("step-model-label"), "test-model");
  let agent = editor
    .stored(id)
    .steps()
    .iter()
    .find(|step| matches!(step.kind, StepKind::Agent { .. }))
    .expect("the agent step")
    .key
    .clone();
  editor.click("pipeline-start");
  editor.click(&format!("pipeline-step-{agent}"));
  assert_eq!(editor.text("step-model-label"), "test-model");
}
