//! The Steps tab's vertical flow: dependency order, parallel steps in one bracket, each route that sends work back
//! with its target, the Deliver step or its absence; and a viewed version that only reads.
use orchester_core::resource::DeliverStep;

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
  // A team project: a local one opens a step in the Pipelines hub's panel, not on this page.
  let mut editor = Editor::team();
  let id = editor.empty("Feature delivery");
  editor.open(id);
  editor.remove_only_step();
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
  editor.click("pipeline-save");
  editor.until("the Deliver step saved as v2", |e| {
    e.version(id) == 2
      && e
        .stored(id)
        .deliver()
        .is_some_and(|deliver| deliver != &DeliverStep::default())
  });
}

#[test]
fn pipelines_viewed_version_reads_each_step() {
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
  assert!(
    !editor.exists("pipeline-restore"),
    "a viewed version is not restored (D-033)"
  );
  editor.click("pipeline-back-draft");
  editor.until("the edit again", |e| !e.exists("pipeline-viewing-banner"));
  assert!(editor.exists("pipeline-add-step"));
}

#[test]
fn pipelines_instructions_error_shows_under_its_field_and_save_stays_unavailable() {
  // A local project: a team page has the team's head line in place of the version line.
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
    "Not saved yet: v2 · used by 0 tasks",
    "an unsaved, invalid edit is named by the number Save would publish alone"
  );
  assert!(editor.exists("pipeline-save"), "Save stays in the header, unavailable");
  editor.click("pipeline-save");
  editor.arm();
  assert!(editor.version(id) == 1, "an invalid edit is not saved");
  editor.click("step-page-back");
  assert!(
    editor.exists("pipeline-field-error"),
    "away from its field, the page says it"
  );
}

#[test]
fn pipelines_flow_add_step_names_by_the_number_it_adds_and_counts_on_from_a_taken_one() {
  let mut editor = Editor::team();
  let id = editor.empty("Names");
  editor.open(id);
  editor.remove_only_step();
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
  let mut editor = Editor::team();
  let id = editor.empty("Cycle");
  editor.open(id);
  editor.remove_only_step();
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

#[test]
fn pipelines_flow_leave_page_leaves_a_cycle_that_cannot_be_saved() {
  use orchester_app::routes::{ProjectPage, Route};
  let mut editor = Editor::team();
  // As the app's `navigate` tool goes: straight through the navigation, from outside the page; to another of the
  // project's pages, then out of the project.
  let tasks = Route::Project(crate::support::project_id(), ProjectPage::Tasks(Default::default()));
  for (name, away) in [("Cycle", tasks), ("Cycle again", Route::AllProjects)] {
    let id = editor.empty(name);
    editor.open(id);
    editor.click("pipeline-add-step");
    editor.click("pipeline-add-step");
    editor.click("pipeline-step-step");
    editor.click("step-after-picker");
    editor.click("step-after-step-2");
    editor.arm();
    assert!(editor.exists("step-after-menu"));
    editor.window.navigation().go(away.clone());
    editor.window.pass();
    assert!(editor.exists("pipeline-leave-question"), "{away:?}");
    // An open menu would take the press on Leave page to close itself.
    editor.arm();
    assert!(
      !editor.exists("step-after-menu"),
      "the picker closes while the page asks"
    );
    editor.click("pipeline-leave-confirm");
    editor.window.until("left", |w| w.navigation().route_now() == away);
    editor.arm();
    assert_eq!(editor.window.navigation().route_now(), away, "Leave page stays left");
  }
}
