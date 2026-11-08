//! Pipelines through the real headless window: page navigation, drafts, validation and publishing.
mod pipelines_support;
mod support;

use orchester_app::routes::ProjectPage;
use orchester_core::{
  resource::{Pipeline, PipelineEdit},
  store::ResourceStore,
};
use pipelines_support::Editor;

#[test]
fn pipelines_new_is_a_page_and_empty_draft_survives_reopening() {
  let mut editor = Editor::new();
  editor.go(ProjectPage::Pipelines);
  editor.until("empty library", |e| e.exists("pipelines-empty"));
  editor.click("pipeline-new");
  editor.set("pipeline-name", "Dependency updates");
  editor.set("pipeline-description", "Keep dependencies current.");
  editor.click("pipeline-start-2");
  editor.click("pipeline-create");
  editor.until("draft editor", |e| e.exists("pipeline-add-step"));
  let id = editor.selected();
  let draft = editor.db().draft::<Pipeline>(id).unwrap().unwrap();
  assert!(draft.body().steps().is_empty());
  assert_eq!(draft.body().description(), "Keep dependencies current.");
  editor.click("pipeline-publish");
  assert!(editor.text("screen-pipelines").contains("Add a step before publishing"));
  assert!(!editor.exists("pipeline-publish-confirm"));
  assert!(
    editor.exists("pipeline-use-starter"),
    "an empty draft offers its starter steps"
  );
  editor.click("pipeline-parent");
  editor.until("library row", |e| e.exists(&format!("pipeline-row-{id}")));
  assert!(editor.text(&format!("pipeline-row-{id}")).contains("Draft"));
  editor.click(&format!("pipeline-row-{id}"));
  editor.until("reopened editor", |e| e.exists("pipeline-add-step"));
  assert_eq!(editor.selected(), id);
}

#[test]
fn pipelines_steps_keep_fields_validate_and_save() {
  let mut editor = Editor::new();
  let id = editor.empty("Checks");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.set("step-name", "Check code");
  editor.click("step-instructions-row");
  editor.set("step-instructions", "Inspect the changes.");
  editor.click("step-page-back");
  editor.set("step-budget", "4,000");
  editor.click("step-gate");
  editor.click("pipeline-save");
  editor.until("saved step", |e| {
    e.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps().len() == 1
  });
  let draft = editor.db().draft::<Pipeline>(id).unwrap().unwrap();
  let step = &draft.body().steps()[0];
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
    e.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps()[0]
      .budget
      .unwrap()
      .get()
      == 2000
  });
}

#[test]
fn pipelines_publish_weaker_version_asks_in_place_and_preserves_history() {
  let mut editor = Editor::new();
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
  editor.until("weaker draft saved", |e| {
    e.db().draft::<Pipeline>(id).unwrap().is_some()
  });
  editor.click("pipeline-publish");
  editor.until("publish question", |e| e.exists("pipeline-publish-question-confirm"));
  editor.arm();
  assert!(editor.gated());
  editor.click("pipeline-publish-question-confirm");
  editor.until("v2", |e| {
    e.db().latest_version::<Pipeline>(id).unwrap().unwrap().number().get() == 2
  });
  editor.until("published notice", |e| e.exists("pipeline-publish-notice"));
  editor.click("pipeline-publish-notice-close");
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
  editor.until("description saved", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .is_some_and(|d| d.body().description() == "Updated description")
  });
}

#[test]
fn pipelines_slug_collision_and_stale_editor_keep_the_existing_draft() {
  let mut editor = Editor::new();
  let id = editor.empty("Checks");
  editor.go(ProjectPage::NewPipeline);
  editor.until("new pipeline form", |e| e.exists("pipeline-name"));
  editor.set("pipeline-name", "Checks");
  editor.click("pipeline-start-2");
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
  let mut edit = PipelineEdit::new(editor.db().draft::<Pipeline>(id).unwrap().unwrap().body().clone());
  edit.set_description("External");
  let changed = edit.validate().unwrap();
  editor
    .db()
    .save_draft(
      id,
      &orchester_core::resource::Draft::new(changed, None, orchester_core::time::Timestamp::now()),
    )
    .unwrap();
  editor.click("pipeline-save");
  editor.until("conflict", |e| e.exists("pipeline-write-error"));
  assert_eq!(
    editor.db().draft::<Pipeline>(id).unwrap().unwrap().body().description(),
    "External"
  );
  editor.click("pipeline-reload");
  editor.until("reloaded", |e| e.exists("pipeline-description"));
}
#[test]
fn pipelines_agent_and_tool_steps_dependencies_retries_and_narrowing_are_saved() {
  let mut editor = Editor::new();
  editor.profile();
  editor.published(); // Also supplies the published Checks tool.
  let id = editor.empty("Mixed");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.set("step-name", "Inspect");
  editor.choose("step-executor", "Reviewer");
  editor.click("step-open-permissions");
  editor.click("step-ask-tools");
  editor.click("step-page-back");
  editor.click("pipeline-add-step");
  editor.set("step-name", "Verify");
  editor.choose("step-executor", "Checks");
  editor.choose("step-failure", "01 Inspect");
  editor.set("step-retries", "3");
  editor.click("pipeline-step-step");
  editor.click("step-after-picker");
  editor.click("step-after-step-2");
  assert!(
    editor.exists("pipeline-rule-error"),
    "the dependency cycle is explained"
  );
  editor.click("step-after-step-2");
  editor.click("pipeline-save");
  editor.until("tool approval", |e| e.exists("pipeline-save-question-confirm"));
  editor.click("pipeline-save-question-confirm");
  editor.until("mixed draft saved", |e| {
    e.db().draft::<Pipeline>(id).unwrap().unwrap().body().steps().len() == 2
  });
  let draft = editor.db().draft::<Pipeline>(id).unwrap().unwrap();
  let steps = draft.body().steps();
  let orchester_core::resource::StepKind::Agent { permissions, .. } = &steps[0].kind else {
    panic!("agent")
  };
  assert!(permissions.interactive_tool_approvals);
  assert!(matches!(
    steps[1].kind,
    orchester_core::resource::StepKind::Tool { tool: Some(_) }
  ));
  assert!(steps[1].depends_on.contains(&steps[0].key));
  let route = steps[1].on_failure.as_ref().unwrap();
  assert_eq!(route.return_to, steps[0].key);
  assert_eq!(route.max_retries.get(), 3);
}
#[test]
fn pipelines_valid_changes_autosave_and_published_history_is_read_only() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.set("pipeline-description", "Automatically saved");
  editor.until("automatic draft save", |e| {
    e.db()
      .draft::<Pipeline>(id)
      .unwrap()
      .is_some_and(|d| d.body().description() == "Automatically saved")
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
#[path = "pipelines/base_moves.rs"]
mod base_moves;
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
#[path = "pipelines/permissions.rs"]
mod permissions;
#[path = "pipelines/regressions.rs"]
mod regressions;
#[path = "pipelines/reload_pending.rs"]
mod reload_pending;

#[path = "pipelines/publication_states.rs"]
mod publication_states;

#[path = "pipelines/profile_navigation.rs"]
mod profile_navigation;

#[path = "pipelines/publication_notice.rs"]
mod publication_notice;

#[path = "pipelines/watch_pileup.rs"]
mod watch_pileup;

#[path = "pipelines/sources.rs"]
mod sources;

#[path = "pipelines/step_name.rs"]
mod step_name;
