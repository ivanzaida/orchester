//! The Steps tab's vertical flow: dependency order, parallel steps in one bracket, each route that sends work back
//! with its target, the Deliver step or its absence; Publish waiting for a saved draft; and a viewed version that only
//! reads, restored at once while no draft exists.
use orchester_core::{
  resource::{DeliverStep, Pipeline},
  store::ResourceStore,
};

use crate::pipelines_support::Editor;

fn top(editor: &mut Editor, id: &str) -> f32 {
  editor
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("laid out: {id}"))
    .y
}

#[test]
fn pipelines_flow_shows_steps_in_order_parallel_ones_together_and_their_routes() {
  let mut editor = Editor::new();
  let id = editor.empty("Feature delivery");
  editor.open(id);
  editor.click("pipeline-use-starter");
  editor.until("starter steps", |e| e.exists("pipeline-step-implement"));
  let order = [
    "pipeline-start",
    "pipeline-step-research",
    "pipeline-step-plan",
    "pipeline-step-implement",
  ];
  for pair in order.windows(2) {
    assert!(top(&mut editor, pair[0]) < top(&mut editor, pair[1]), "{pair:?}");
  }
  let (checks, review) = (
    top(&mut editor, "pipeline-step-run-checks"),
    top(&mut editor, "pipeline-step-review"),
  );
  assert!(checks > top(&mut editor, "pipeline-step-implement"));
  assert!(review > checks, "parallel steps stack in their bracket");
  let bracket = editor.text("pipeline-parallel");
  assert!(bracket.contains("RUN IN PARALLEL"));
  assert!(bracket.contains("Run checks") && bracket.contains("Review"));
  assert!(
    !bracket.contains("Implement ·"),
    "only the level's own steps are in the bracket"
  );
  assert_eq!(
    editor.text("pipeline-failure-route-review"),
    "Changes requested → 03 Implement"
  );
  assert!(editor.text("pipeline-step-research").starts_with("01"));
  assert!(editor.text("pipeline-step-review").contains("05"));
  assert!(
    top(&mut editor, "pipeline-add-step") > review,
    "Add step follows the steps"
  );
}

#[test]
fn pipelines_flow_without_deliver_step_offers_to_add_one() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  assert!(
    editor.exists("pipeline-deliver-chip"),
    "no Deliver step says where runs end"
  );
  assert!(
    editor
      .text("pipeline-deliver-chip")
      .contains("No Deliver step · runs end in Ready for review")
  );
  assert!(!editor.exists("pipeline-deliver"));
  editor.click("pipeline-deliver-chip");
  assert!(editor.text("deliver-none").contains("No Deliver step"));
  editor.click("deliver-add");
  editor.until("the Deliver step is shown", |e| e.exists("pipeline-deliver"));
  assert!(!editor.exists("pipeline-deliver-chip"));
  assert!(
    editor.text("pipeline-deliver").starts_with("02"),
    "Deliver is numbered last"
  );
  assert!(editor.exists("deliver-mode-approve"));
  assert!(editor.text("deliver-mode-pull-request").contains("Coming later"));
  editor.click("deliver-mode-auto");
  editor.until("the Deliver step saved", |e| {
    e.db().draft::<Pipeline>(id).unwrap().is_some_and(|draft| {
      draft
        .body()
        .deliver()
        .is_some_and(|deliver| deliver != &DeliverStep::default())
    })
  });
}

#[test]
fn pipelines_publish_waits_while_changes_are_unsaved() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  assert!(
    editor.exists("pipeline-header-history"),
    "a published pipeline with no draft offers its history"
  );
  editor.click("pipeline-step-step");
  editor.click("step-gate");
  assert_eq!(editor.text("pipeline-save-status"), "Unsaved changes");
  assert!(editor.exists("pipeline-save"));
  editor.click("pipeline-publish");
  assert!(
    !editor.exists("pipeline-publish-review"),
    "Publish is disabled while changes are unsaved"
  );
  editor.click("pipeline-save");
  editor.until("save question", |e| e.exists("pipeline-save-question-confirm"));
  editor.click("pipeline-save-question-confirm");
  editor.until("draft saved", |e| e.db().draft::<Pipeline>(id).unwrap().is_some());
  editor.until("save status", |e| {
    e.text("pipeline-save-status") == "Draft saved · Published v1"
  });
  editor.click("pipeline-publish");
  assert!(
    editor.exists("pipeline-publish-review"),
    "a saved draft can be reviewed"
  );
  assert!(editor.text("pipeline-publish-review").contains("1 change since v1"));
  assert!(
    editor.exists("pipeline-step-changed-step"),
    "the review tags the changed step on the flow"
  );
}

#[test]
fn pipelines_viewed_version_reads_each_step_and_restores_at_once_without_a_draft() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-history");
  editor.until("versions", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  editor.until("the viewed flow", |e| e.exists("pipeline-viewing-banner"));
  assert_eq!(editor.text("pipeline-save-status"), "Viewing v1 · read-only");
  assert!(!editor.exists("pipeline-add-step"), "a viewed version adds no step");
  assert!(!editor.exists("pipeline-save"));
  editor.click("pipeline-step-step");
  assert!(!editor.exists("step-duplicate"), "a viewed step reads, never edits");
  assert!(!editor.exists("step-executor"));
  assert_eq!(editor.text("step-executor-row-value"), "Checks");
  editor.click("pipeline-restore");
  assert!(
    !editor.exists("pipeline-restore-question"),
    "with no draft, nothing is replaced, so nothing is asked"
  );
  assert!(
    !editor.exists("pipeline-viewing-banner"),
    "the restored version is the draft now"
  );
  assert!(editor.exists("pipeline-add-step"));
}

#[test]
fn pipelines_instructions_error_shows_under_its_field_and_keeps_save_draft() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-step-step");
  editor.click("step-instructions-row");
  editor.set("step-instructions", "Check {{task.brief");
  assert!(
    editor.text("screen-pipelines").contains("never closed"),
    "the field says what is wrong"
  );
  assert!(
    !editor.exists("pipeline-field-error"),
    "the error is said once, under the field"
  );
  assert_eq!(editor.text("pipeline-save-status"), "Unsaved changes");
  assert_eq!(
    editor.text("pipeline-version-detail"),
    "Draft v2 · used by 0 tasks",
    "an unsaved, invalid draft is named by its number alone"
  );
  assert!(editor.exists("pipeline-save"), "Save draft stays in the header");
  editor.click("pipeline-save");
  editor.arm();
  assert!(
    editor.db().draft::<Pipeline>(id).unwrap().is_none(),
    "an invalid draft is not saved"
  );
  editor.click("step-page-back");
  assert!(
    editor.exists("pipeline-field-error"),
    "away from its field, the page says it"
  );
}

#[test]
fn pipelines_flow_add_step_names_by_the_number_it_adds_and_counts_on_from_a_taken_one() {
  let mut editor = Editor::new();
  let id = editor.empty("Names");
  editor.open(id);
  editor.click("pipeline-add-step");
  assert_eq!(editor.value("step-name"), "New step 1");
  editor.click("pipeline-add-step");
  assert_eq!(editor.value("step-name"), "New step 2");
  editor.set("step-name", "New step 3");
  editor.click("pipeline-add-step");
  assert_eq!(
    editor.value("step-name"),
    "New step 4",
    "the third step's name is taken: the next number after it, never back to a lower one"
  );
}

#[test]
fn pipelines_flow_a_step_on_a_cycle_says_so_on_its_runs_after_row() {
  let mut editor = Editor::new();
  let id = editor.empty("Cycle");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.click("pipeline-add-step");
  assert!(!editor.exists("step-dependencies-hint"));
  editor.click("pipeline-step-step");
  editor.click("step-after-picker");
  editor.click("step-after-step-2");
  // `A cycle` `Igf5o` in `g6Ecy`: the hint on the Runs after row.
  assert_eq!(
    editor.text("step-dependencies-hint"),
    "New step 1 waits for itself through the steps it runs after"
  );
  editor.click("step-after-step-2");
  assert!(!editor.exists("step-dependencies-hint"), "the hint goes with the cycle");
}
