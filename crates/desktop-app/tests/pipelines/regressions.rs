//! Review regressions: incomplete starters, input identity and leaving with pending edits.
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  resource::{Pipeline, RepositoryAccess, StepKind},
  store::ResourceStore,
};

use crate::pipelines_support::Editor;

#[test]
fn pipelines_unassigned_starter_saves_edits_but_cannot_publish() {
  let mut editor = Editor::new();
  editor.go(ProjectPage::NewPipeline);
  editor.until("new pipeline", |e| e.exists("pipeline-name"));
  editor.set("pipeline-name", "Delivery");
  editor.click("pipeline-create");
  editor.until("starter", |e| e.exists("pipeline-step-implement"));
  let id = editor.selected();
  editor.set("pipeline-description", "An incomplete starter");
  editor.until("starter saved", |e| {
    e.db().draft::<Pipeline>(id).unwrap().unwrap().body().description() == "An incomplete starter"
  });
  editor.click("pipeline-publish");
  assert!(editor.exists("pipeline-publish-problem"));
  assert!(!editor.exists("pipeline-publish-confirm"));
  assert!(editor.db().latest_version::<Pipeline>(id).unwrap().is_none());
  editor.click("pipeline-step-implement");
  assert!(editor.exists("step-write"));
  editor.click("step-read");
  editor.until("write removed", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .body()
      .steps()
      .iter()
      .any(|step| {
        step.key.as_str() == "implement"
          && matches!(
            step.kind,
            StepKind::Agent {
              repository: RepositoryAccess::ReadOnly,
              ..
            }
          )
      })
  });
}

#[test]
fn pipelines_keyboard_focus_caret_and_undo_survive_autosave() {
  let mut editor = Editor::new();
  let id = editor.empty("Typing");
  editor.open(id);
  keyboard_across_save(&mut editor, "pipeline-description", "a");
  editor.click("pipeline-settings");
  keyboard_across_save(&mut editor, "pipeline-settings-description", "d");
  editor.click("pipeline-tab-steps");
  editor.click("pipeline-add-step");
  editor.set("step-name", "Typing");
  editor.click("step-instructions-row");
  keyboard_across_save(&mut editor, "step-instructions", "g");
}

fn keyboard_across_save(editor: &mut Editor, field: &str, first: &str) {
  editor.set(field, "");
  editor.focus(field);
  // Position the caret immediately before the save, independently of insertion behavior.
  for key in [first, "b", "c", "ArrowLeft"] {
    editor.key(key, false);
  }
  let typed = format!("{first}bc");
  assert_eq!(editor.value(field), typed);
  assert!(editor.exists("pipeline-save"), "each field must cause a new save");
  editor.until("autosave completed", |e| {
    !e.exists("pipeline-save") && !e.text("screen-pipelines").contains("Saving draft")
  });
  let saved = editor
    .db()
    .draft::<Pipeline>(editor.selected())
    .expect("read the saved pipeline")
    .expect("autosave created a draft");
  let saved_text = if field == "step-instructions" {
    saved.body().steps()[0].instructions.as_str()
  } else {
    saved.body().description()
  };
  assert_eq!(saved_text, typed, "the typed value reached the database");
  assert_eq!(
    editor
      .window
      .tree
      .focused_element()
      .and_then(|e| e.id().map(str::to_owned))
      .as_deref(),
    Some(field)
  );
  // No refocus: typing resumes at the same caret, and Undo reaches the pre-save history.
  editor.key("y", false);
  assert_eq!(
    editor.value(field),
    format!("{first}byc"),
    "the caret stays in the middle"
  );
  editor.key("z", true);
  assert_eq!(editor.value(field), typed);
  editor.key("z", true);
  assert_eq!(editor.value(field), format!("{first}b"));
}

#[test]
fn pipelines_leaving_before_autosave_requires_a_decision_and_keep_preserves_edits() {
  let mut editor = Editor::new();
  let id = editor.empty("Pending");
  editor.go(ProjectPage::Pipelines);
  editor.open(id);
  editor.set("pipeline-description", "Pending description");
  editor.click("pipeline-parent");
  assert_eq!(editor.selected(), id);
  assert!(editor.exists("pipeline-leave-question"));
  assert_eq!(editor.value("pipeline-description"), "Pending description");
  assert_eq!(
    editor.db().draft::<Pipeline>(id).unwrap().unwrap().body().description(),
    ""
  );
  editor.click("pipeline-leave-keep");
  assert!(!editor.exists("pipeline-leave-question"));
  editor.until("kept work saved", |e| {
    e.db().draft::<Pipeline>(id).unwrap().unwrap().body().description() == "Pending description"
  });
  editor.click("pipeline-parent");
  editor.until("library", |e| e.exists(&format!("pipeline-row-{id}")));
  editor.open(id);
  assert_eq!(editor.value("pipeline-description"), "Pending description");
}

#[test]
fn pipelines_sidebar_history_and_tabs_preserve_unconfirmed_and_invalid_edits() {
  let mut editor = Editor::new();
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
  assert!(editor.db().draft::<Pipeline>(id).unwrap().is_none());
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
  assert!(
    editor.db().draft::<Pipeline>(id).unwrap().is_none(),
    "explicit discard never saves"
  );
  editor.open(id);
  editor.click("pipeline-step-step");
  assert_eq!(editor.value("step-budget"), "No limit");
  editor.click("pipeline-parent");
  editor.until("clean departure", |e| e.exists(&format!("pipeline-row-{id}")));
}

#[test]
fn pipelines_failure_route_reset_keeps_displayed_and_saved_retries_in_sync() {
  let mut editor = Editor::new();
  let id = editor.empty("Retries");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.set("step-name", "Inspect");
  editor.click("pipeline-add-step");
  editor.set("step-name", "Verify");
  editor.choose("step-failure", "01 Inspect");
  editor.set("step-retries", "3");
  editor.until("three retries saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .body()
      .steps()
      .get(1)
      .and_then(|step| step.on_failure.as_ref())
      .is_some_and(|route| route.max_retries.get() == 3)
  });
  editor.choose("step-failure", "Fail the run");
  assert!(!editor.exists("step-retries"));
  editor.choose("step-failure", "01 Inspect");
  assert_eq!(editor.value("step-retries"), "1");
  editor.click("pipeline-start");
  editor.click("pipeline-step-step-2");
  assert_eq!(editor.value("step-retries"), "1", "cached text also resets");
  editor.until("one retry saved", |e| {
    e.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps()[1]
      .on_failure
      .as_ref()
      .is_some_and(|route| route.max_retries.get() == 1)
  });
  assert_eq!(editor.value("step-retries"), "1");
}

#[test]
fn pipelines_removing_failure_route_clears_obsolete_retry_errors() {
  let mut editor = Editor::new();
  let id = editor.empty("Invalid retries");
  editor.open(id);
  for name in ["Inspect", "Repair", "Verify"] {
    editor.click("pipeline-add-step");
    editor.set("step-name", name);
  }
  editor.choose("step-failure", "01 Inspect");
  editor.set("step-retries", "3");
  editor.choose("step-failure", "02 Repair");
  assert_eq!(
    editor.value("step-retries"),
    "3",
    "changing only the target keeps retries"
  );
  editor.set("step-retries", "invalid");
  editor.choose("step-failure", "01 Inspect");
  assert_eq!(
    editor.value("step-retries"),
    "invalid",
    "keep unfinished edits on target changes"
  );
  assert!(editor.exists("pipeline-field-error"));
  editor.choose("step-failure", "Fail the run");
  assert!(!editor.exists("pipeline-field-error"));
  editor.click("pipeline-start");
  editor.click("pipeline-step-step-3");
  editor.choose("step-failure", "02 Repair");
  assert_eq!(editor.value("step-retries"), "1");
  editor.until("valid route saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .body()
      .steps()
      .get(2)
      .and_then(|step| step.on_failure.as_ref())
      .is_some_and(|route| route.max_retries.get() == 1 && route.return_to.as_str() == "step-2")
  });
}

#[test]
fn pipelines_executor_reset_keeps_displayed_and_saved_model_in_sync() {
  let mut editor = Editor::new();
  editor.profile();
  editor.published(); // Supplies the Checks tool.
  let id = editor.empty("Model transitions");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.choose("step-executor", "Reviewer");
  editor.choose("step-model", "test-model");
  editor.until("model override saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .unwrap()
      .body()
      .steps()
      .first()
      .is_some_and(|step| {
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
  assert!(editor.text("step-model").contains("From the profile"));
  editor.until("inherited model saved", |e| {
    matches!(
      e.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps()[0].kind,
      StepKind::Agent {
        model: None,
        profile: Some(_),
        ..
      }
    )
  });
  assert!(editor.text("step-model").contains("From the profile"));
  editor.click("pipeline-start");
  editor.click("pipeline-step-step");
  assert!(editor.text("step-model").contains("From the profile"));
}
