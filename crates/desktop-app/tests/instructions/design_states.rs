//! Behavioral and structural regressions for the returned fePhH findings.
use orchester_core::resource::InstructionScope;
use orchester_workspace::instructions::{editor, publication::Unavailable};

use super::{harness::Instructions, preview::seed_preview};

#[test]
fn instructions_import_is_a_dedicated_page_with_metadata_and_cancel() {
  let mut screen = Instructions::new();
  std::fs::write(screen.root.join("AGENTS.md"), "Keep changes small.\n").expect("private fixture");
  std::fs::write(screen.root.join("CLAUDE.md"), "@AGENTS.md").expect("private fixture");
  screen.go();
  screen.click("instructions-import");
  screen.until("review", |s| s.exists("instruction-import-create"));
  // A page of its own under the hub's tabs (02c): the list is not beside it.
  assert!(!screen.exists("instruction-library") && !screen.exists("instructions-empty"));
  let review = screen.text("instruction-import-review");
  assert!(review.contains("INSTRUCTIONS"));
  assert!(review.contains("Draft project instruction AGENTS · v1"));
  assert!(review.contains("Project · change it in the editor"));
  assert!(review.contains("All agents · Agent start"));
  assert!(review.contains("Reference files"));
  assert!(screen.text("instruction-import-source").contains("1 line · read only"));
  assert!(screen.text("instruction-import-selection").contains("1 of 2 selected"));
  screen.click("instruction-import-cancel");
  screen.until("return", |s| s.exists("instructions-empty"));
  assert!(screen.listed().entries.is_empty());
}

#[test]
fn instructions_missing_value_locks_publication_and_rechecks_the_event_in_workspace() {
  let mut screen = Instructions::new();
  seed_preview(&screen);
  let id = screen.rule(
    "Needs checkpoint",
    "Compare {{run.revision}}",
    InstructionScope::Project,
    false,
  );
  screen.go();
  screen.until("missing value", |s| s.exists("instruction-publication-warning"));
  assert!(
    screen
      .text("instruction-publication-warning")
      .contains("{{run.revision}} has no value at Agent start")
  );
  screen.click("instruction-publish");
  screen.pump();
  assert!(screen.entry(id).state.published().is_none());
  let refused = editor::publish_in(&screen.root, &screen.entry(id)).expect_err("the revision has no value at start");
  assert!(matches!(
    refused,
    editor::WriteError::Unavailable(Unavailable {
      variable: orchester_core::resource::Variable::RunRevision,
      event: orchester_core::resource::InjectionEvent::Start,
    })
  ));
  assert_eq!(refused.message().key, "instructions.errors.variableUnavailableAtStart");
  screen.set("instruction-prompt", "Working on {{task.title}}");
  screen.until("valid draft saved", |s| {
    s.entry(id).body.text().as_str() == "Working on {{task.title}}"
  });
  screen.until("warning clears", |s| !s.exists("instruction-publication-warning"));
  screen.click("instruction-publish");
  screen.until("published", |s| s.exists("instruction-preview-prompt"));
  assert_eq!(screen.entry(id).state.published().expect("published").get(), 1);
  assert!(screen.exists("instruction-published-notice"));
  screen.click("instruction-published-dismiss");
  assert!(!screen.exists("instruction-published-notice"));
}

#[test]
fn instructions_preview_keeps_horizontal_actions_visible_and_viewer_is_read_only() {
  let mut screen = Instructions::new();
  seed_preview(&screen);
  screen.rule(
    "Standards",
    "Working on {{task.title}}",
    InstructionScope::Project,
    false,
  );
  screen.go();
  screen.click("instruction-panel-variables");
  screen.until("variable context", |s| s.exists("instruction-variable-context"));
  assert!(screen.text("instruction-variable-context").contains("KON-1 · Builder"));
  assert!(screen.text("instruction-variable-detail").contains("Available"));
  screen.click("instruction-panel-preview");
  screen.until("preview", |s| s.exists("instruction-full-prompt"));
  let view = screen
    .window
    .tree
    .get_element_by_id_mut("instruction-full-prompt")
    .expect("view")
    .bounds()
    .expect("view bounds");
  let copy = screen
    .window
    .tree
    .get_element_by_id_mut("instruction-copy-prompt")
    .expect("copy")
    .bounds()
    .expect("copy bounds");
  assert!((view.y - copy.y).abs() < 1.0, "preview actions share one row");
  assert!(
    copy.x > view.x && copy.y + copy.height < 875.0,
    "actions fit inside the page"
  );
  // The source row's sub-line names its part, its version and that it is this rule (`fSWIV` in 02b).
  let panel = screen.text("instruction-panel");
  assert!(panel.contains("Project rule · Draft v1 · this rule"), "{panel}");
  screen.click("instruction-full-prompt");
  screen.until("viewer", |s| s.exists("instruction-full-viewer"));
  assert!(
    screen
      .text("instruction-viewer-context")
      .contains("Kontur / KON-1 · Builder · Implement · Agent start")
  );
  assert!(
    screen
      .text("instruction-full-viewer")
      .contains("PROJECT RULE · Standards v1")
  );
  assert!(
    screen
      .text("instruction-viewer-summary")
      .contains("1 variable resolved")
  );
  screen
    .window
    .tree
    .get_element_by_id_mut("instruction-viewer-text-1")
    .expect("source")
    .focus();
  screen.key("a", "KeyA", false, true);
  screen.key("x", "KeyX", false, false);
  assert_eq!(
    screen.text("instruction-viewer-text-1"),
    "Working on Add retry to webhook sender"
  );
  screen.key("Escape", "Escape", false, false);
  screen.until("back", |s| s.exists("instruction-prompt"));
}

#[test]
fn instructions_role_and_step_editors_show_their_injection_explanations() {
  let mut screen = Instructions::new();
  let pipeline = seed_preview(&screen);
  let step = screen.listed().pipeline(pipeline).expect("pipeline").steps[0]
    .key
    .clone();
  screen.rule(
    "Builder rule",
    "Role rule",
    InstructionScope::AgentRole {
      role: orchester_core::name::Name::new("Builder").expect("role"),
    },
    true,
  );
  screen.rule(
    "Step rule",
    "Step rule",
    InstructionScope::Step { pipeline, step },
    true,
  );
  screen.list();
  let list = screen.text("instruction-library");
  assert!(
    list.contains("Feature delivery v1") && list.contains("01 Implement"),
    "{list}"
  );
  screen.click("instruction-tab-1");
  screen.until("role editor", |s| s.exists("instruction-role"));
  assert!(
    screen
      .text("instruction-editor")
      .contains("after the profile’s own role instructions")
  );
  screen.click("instruction-tab-3");
  screen.until("step editor", |s| s.exists("instruction-step"));
  assert!(!screen.exists("instruction-targets"));
  assert!(screen.text("instruction-editor").contains("01 Implement runs Builder"));
}

#[test]
fn instructions_removing_target_chips_removes_the_named_role_and_empty_selection_blocks_saving() {
  use orchester_core::{
    id::ConnectionId,
    name::Name,
    resource::{AgentProfile, ModelId, PromptText, ProviderSettings, Slug},
    store::ResourceStore,
    time::Timestamp,
  };
  let mut screen = Instructions::new();
  seed_preview(&screen);
  let profile = AgentProfile::new(
    Name::new("Reviewer").expect("role"),
    "Reviews changes".into(),
    PromptText::new("Review").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("test").expect("model"),
    ProviderSettings::default(),
  );
  orchester_store::ProjectDb::open(&screen.root)
    .expect("fixture")
    .create_resource(
      Slug::new("reviewer").expect("slug"),
      Name::new("Reviewer").expect("name"),
      &profile,
      Timestamp::now(),
    )
    .expect("profile");
  let id = screen.rule("Standards", "For selected roles", InstructionScope::Project, true);
  screen.go();
  screen.choose("instruction-targets", "Reviewer");
  screen.choose("instruction-targets", "Builder");
  screen.until(
    "two roles saved",
    |s| matches!(s.entry(id).body.targets(), orchester_core::resource::AgentTargets::Roles(roles) if roles.len() == 2),
  );
  let removed = screen.text("instruction-target-0");
  screen.click("instruction-target-0-remove");
  screen.until("named role removed", |s| matches!(s.entry(id).body.targets(),
    orchester_core::resource::AgentTargets::Roles(roles) if roles.len() == 1 && !roles.iter().any(|role| role.as_str() == removed)));
  let before = screen.entry(id);
  screen.click("instruction-target-0-remove");
  assert!(screen.exists("instruction-settings-error"));
  assert!(screen.text("instruction-settings-error").contains("Choose the agents"));
  screen.click("instruction-publish");
  screen.pump();
  assert_eq!(
    screen.entry(id),
    before,
    "an empty Apply to field neither saves nor publishes"
  );
}
#[test]
fn instructions_full_viewer_can_return_when_its_task_loses_preview_context() {
  use orchester_app::root::Start;
  use orchester_core::store::TaskStore;
  use orchester_workspace::tasks::outside::Outside;
  let mut screen = Instructions::new();
  seed_preview(&screen);
  screen.rule(
    "Standards",
    "Working on {{task.title}}",
    InstructionScope::Project,
    false,
  );
  screen.go();
  screen.click("instruction-panel-preview");
  screen.until("preview", |s| s.exists("instruction-full-prompt"));
  screen.click("instruction-full-prompt");
  screen.until("viewer", |s| s.exists("instruction-full-viewer"));
  let mut db = orchester_store::ProjectDb::open(&screen.root).expect("private fixture");
  let task = db.tasks().expect("tasks")[0].clone();
  let mut details = task.details().clone();
  details.pipeline = None;
  assert!(
    db.update_task(task.id(), task.revision(), details)
      .expect("task changed")
      .saved()
      .is_some()
  );
  let Start::Ready(ready) = screen.window.launch.start() else {
    panic!("ready fixture");
  };
  ready
    .services
    .changes
    .record(super::support::project_id(), Outside::Tasks);
  screen.until("viewer unavailable", |s| s.exists("instruction-viewer-unavailable"));
  assert!(screen.exists("instruction-preview-task"));
  screen.click("instruction-viewer-back");
  screen.until("editor", |s| s.exists("instruction-prompt"));
  assert!(screen.exists("agents-hub-tabs"));
  let changed = db.task(task.id()).expect("task").expect("present");
  assert!(
    db.update_task(task.id(), changed.revision(), task.details().clone())
      .expect("context restored")
      .saved()
      .is_some()
  );
  ready
    .services
    .changes
    .record(super::support::project_id(), Outside::Tasks);
  screen.until("preview recovered", |s| s.exists("instruction-full-prompt"));
  screen.click("instruction-full-prompt");
  screen.until("viewer recovered", |s| s.exists("instruction-full-viewer"));
}
