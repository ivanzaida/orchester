//! Pipelines through the real headless window: page navigation, validation and saving, which publishes the next
//! version (a pipeline holds no draft, D-032).
mod pipelines_support;
mod support;

use orchester_app::routes::{PipelineView, ProjectPage};
use orchester_core::{
  resource::{Pipeline, StepKey},
  store::ResourceStore,
};
use pipelines_support::Editor;

#[test]
fn pipelines_steps_keep_fields_validate_and_save() {
  let mut editor = Editor::team();
  let id = editor.empty("Checks");
  editor.open(id);
  // The fixture's one step, which runs a tool: a step added here would run nothing, which saving refuses.
  editor.click("pipeline-step-step");
  editor.set("step-name", "Check code");
  editor.click("step-instructions-row");
  editor.set("step-instructions", "Inspect the changes.");
  editor.click("step-page-back");
  editor.set("step-budget", "4,000");
  editor.click("step-gate");
  editor.click("pipeline-save");
  editor.until("saved step", |e| e.version(id) == 2);
  let body = editor.stored(id);
  let step = &body.steps()[0];
  assert_eq!(step.name.as_str(), "Check code");
  assert!(step.approval_gate);
  assert_eq!(step.budget.unwrap().get(), 4000);
  editor.set("step-budget", "bad");
  editor.click("pipeline-start");
  editor.click(&format!("pipeline-step-{}", step.key));
  assert!(editor.exists("pipeline-field-error"));
  editor.set("step-budget", "2,000");
  editor.click("step-duplicate");
  editor.arm();
  editor.click("step-remove");
  editor.until("remove question", |e| e.exists("step-remove-confirm"));
  editor.click("step-remove-confirm");
  editor.click("pipeline-save");
  editor.until("saved budget", |e| {
    let body = e.stored(id);
    body.steps().len() == 1 && body.steps()[0].budget.is_some_and(|budget| budget.get() == 2000)
  });
}

#[test]
fn pipelines_saving_a_weaker_version_asks_in_place_and_preserves_history() {
  let mut editor = Editor::team();
  let id = editor.published();
  editor.open(id);
  let step = editor
    .db()
    .latest_version::<Pipeline>(id)
    .unwrap()
    .unwrap()
    .body()
    .steps()[0]
    .key
    .clone();
  editor.click(&format!("pipeline-step-{step}"));
  editor.click("step-gate");
  editor.click("pipeline-save");
  editor.until("save question armed", |e| e.exists("pipeline-save-question-confirm"));
  editor.arm();
  assert!(editor.gated());
  editor.click("pipeline-save-question-confirm");
  editor.until("the weaker version saved as v2", |e| e.version(id) == 2);
  let versions = editor.db().versions::<Pipeline>(id).unwrap();
  assert!(versions[0].body().steps()[0].approval_gate);
  assert!(!versions[1].body().steps()[0].approval_gate);
  editor.click("pipeline-history");
  editor.until("version list", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  editor.click("pipeline-back-draft");
  editor.click("pipeline-settings");
  assert!(editor.exists("pipeline-settings-page"));
  editor.set("pipeline-settings-description", "Updated description");
  editor.click("pipeline-tab-steps");
  editor.click("pipeline-save");
  editor.until("description saved as v3", |e| {
    e.version(id) == 3 && e.stored(id).description() == "Updated description"
  });
}

#[test]
fn pipelines_slug_collision_and_stale_editor_keep_the_latest_version() {
  let mut editor = Editor::new();
  let id = editor.empty("Checks");
  editor.go(ProjectPage::NewPipeline);
  editor.until("new pipeline form", |e| e.exists("pipeline-name"));
  editor.set("pipeline-name", "Checks");
  editor.click("pipeline-create");
  editor.until("collision", |e| e.exists("pipeline-open-existing"));
  assert!(editor.text("pipeline-slug-hint").contains("Already used by Checks"));
  assert!(
    editor
      .text("screen-pipelines")
      .contains("Choose a name with a free slug.")
  );
  editor.open(id);
  editor.set("pipeline-description", "Local");
  // Another editor publishes the next version behind the page.
  editor.publish_behind(id, "External");
  editor.click("pipeline-save");
  editor.until("conflict", |e| e.exists("pipeline-write-error"));
  assert_eq!(editor.stored(id).description(), "External");
  editor.click("pipeline-reload");
  editor.until("reloaded", |e| e.exists("pipeline-description"));
}
#[test]
fn pipelines_valid_changes_autosave_and_published_history_is_read_only() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.set("pipeline-description", "Automatically saved");
  editor.until("automatic save as v2", |e| {
    e.version(id) == 2 && e.stored(id).description() == "Automatically saved"
  });
  editor.click("pipeline-history");
  editor.until("history", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  assert!(editor.text("screen-pipelines").contains("Viewing v1"));
  editor.click("pipeline-step-step");
  assert!(
    !editor.exists("step-gate"),
    "a viewed version reads its approval, never switches it"
  );
  assert_eq!(editor.text("step-gate-row-value"), "On");
  assert!(!editor.exists("pipeline-save"), "a viewed version has nothing to save");
  editor.click("pipeline-back-draft");
  assert!(!editor.text("screen-pipelines").contains("Viewing v1"));
}

#[path = "pipelines/async_failures.rs"]
mod async_failures;
#[path = "pipelines/default.rs"]
mod default;
#[path = "pipelines/delivers.rs"]
mod delivers;
#[path = "pipelines/flow.rs"]
mod flow;
#[path = "pipelines/history_safety.rs"]
mod history_safety;
#[path = "pipelines/layout.rs"]
mod layout;
#[path = "pipelines/panel_history.rs"]
mod panel_history;
#[path = "pipelines/regressions.rs"]
mod regressions;
#[path = "pipelines/reload_pending.rs"]
mod reload_pending;

#[path = "pipelines/publication_states.rs"]
mod publication_states;

#[path = "pipelines/watch_pileup.rs"]
mod watch_pileup;

#[path = "pipelines/sources.rs"]
mod sources;

#[path = "pipelines/step_name.rs"]
mod step_name;

#[path = "pipelines/one_save.rs"]
mod one_save;

/// The Pipelines hub's panel shows a step of a local project; a team project's pipelines are edited on the pages they
/// have, with the step's own column.
#[test]
fn pipelines_a_team_projects_pipeline_and_step_pages_are_the_editors_own() {
  let mut editor = Editor::team();
  let id = editor.published();
  editor.open(id);
  assert!(!editor.exists("pipeline-hub-step-head"));
  let key = editor
    .db()
    .latest_version::<Pipeline>(id)
    .unwrap()
    .unwrap()
    .body()
    .steps()[0]
    .key
    .clone();

  editor.go(ProjectPage::Pipeline(
    id,
    PipelineView::Step(StepKey::new(key.as_str()).expect("a key")),
  ));
  editor.until("the step's own column", |e| e.exists("step-name"));
  assert!(
    !editor.exists("pipeline-hub-step-head"),
    "a team project's step is not opened in the hub's panel"
  );
  assert!(
    editor.exists("step-executor"),
    "its Executor is still the menu of profiles and tools"
  );
  assert!(
    !editor.exists("pipeline-hub-step-agent"),
    "the Agent picker belongs to the hub's step panel"
  );
}
