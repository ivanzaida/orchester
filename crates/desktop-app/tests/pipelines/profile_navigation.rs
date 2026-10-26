//! Actual current-profile navigation keeps pending decisions and unsaved Pipeline fields intact.
use std::collections::BTreeSet;

use orchester_app::routes::{PipelineView, ProjectPage, Route};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{RepositoryAccess, StepPermissions},
  resource::{AgentProfile, Draft, ModelId, Pipeline, PromptText, ProviderSettings, Slug, Step, StepKey, StepKind},
  store::ResourceStore,
  time::Timestamp,
};

use crate::{pipelines_support::Editor, support::project_id};

fn assigned_profile() -> (Editor, ResourceId, ResourceId, ResourceId) {
  let editor = Editor::new();
  let mut db = editor.db();
  let profile = |name: &str| {
    AgentProfile::new(
      Name::new(name).expect("valid profile role"),
      "Current editable profile".into(),
      PromptText::new("Inspect the approved change.").expect("valid profile instructions"),
      ConnectionId::generate(),
      ModelId::new("test-model").expect("valid fixture model"),
      ProviderSettings::default(),
    )
  };
  let mut create = |name: &str| {
    db.create_resource(
      Slug::from_name(name).expect("valid fixture slug"),
      Name::new(name).expect("valid fixture name"),
      &profile(name),
      Timestamp::now(),
    )
    .expect("draft profile persists")
    .id()
  };
  let assigned = create("Explorer");
  let other = create("Another profile");
  let pipeline = Pipeline::new(vec![Step {
    key: StepKey::new("research").expect("valid step key"),
    name: Name::new("Research").expect("valid step name"),
    kind: StepKind::Agent {
      profile: Some(assigned),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Inspect.").expect("valid step instructions"),
    depends_on: BTreeSet::new(),
    approval_gate: true,
    on_failure: None,
    budget: None,
  }])
  .expect("one assigned pipeline step");
  let id = db
    .create_resource(
      Slug::new("profile-navigation").expect("valid pipeline slug"),
      Name::new("Profile navigation").expect("valid pipeline name"),
      &pipeline,
      Timestamp::now(),
    )
    .expect("pipeline draft persists")
    .id();
  (editor, id, assigned, other)
}
fn open_step(editor: &mut Editor, id: ResourceId) {
  editor.open(id);
  editor.click("pipeline-step-research");
  assert!(editor.exists("step-profile-unpublished"));
  assert!(editor.text("step-open-profile").contains("Open Explorer"));
}
fn stored(editor: &Editor, id: ResourceId) -> Draft<Pipeline> {
  editor
    .db()
    .draft::<Pipeline>(id)
    .expect("pipeline draft reads")
    .expect("pipeline draft exists")
}
fn current_agent(editor: &mut Editor, profile: ResourceId, role: &str) {
  assert_eq!(
    editor.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Agent(profile, Default::default()))
  );
  editor.window.until("current assigned Agent editor", |w| {
    w.tree.get_element_by_id("agent-role").is_some()
  });
  assert_eq!(editor.value("agent-role"), role);
  assert_eq!(editor.value("agent-instructions"), "Inspect the approved change.");
  assert!(editor.text("agent-model").contains("test-model"));
}
fn stays(editor: &Editor, id: ResourceId) {
  let step = PipelineView::Step(StepKey::new("research").expect("valid step key"));
  assert_eq!(
    editor.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Pipeline(id, step))
  );
}

#[test]
fn pipelines_profile_link_opens_exact_current_agent_and_different_identity_remounts() {
  let (mut editor, id, assigned, other) = assigned_profile();
  let before = stored(&editor, id);
  open_step(&mut editor, id);
  editor.click("step-open-profile");
  current_agent(&mut editor, assigned, "Explorer");
  assert_eq!(stored(&editor, id), before, "navigation does not save a pipeline");
  editor.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Agent(other, Default::default()),
  ));
  editor.window.until("other current Agent resource", |w| {
    w.tree
      .get_element_by_id("agent-role")
      .is_some_and(|e| e.text_content() == Some("Another profile"))
  });
  current_agent(&mut editor, other, "Another profile");
}

#[test]
fn pipelines_profile_link_holds_dirty_invalid_fields_and_keep_preserves_them() {
  let (mut editor, id, ..) = assigned_profile();
  let before = stored(&editor, id);
  open_step(&mut editor, id);
  editor.set("step-name", "Unsaved local name");
  editor.set("step-budget", "invalid budget");
  editor.click("step-open-profile");
  stays(&editor, id);
  assert!(editor.exists("pipeline-leave-question"));
  editor.click("pipeline-leave-keep");
  assert!(!editor.exists("pipeline-leave-question"));
  assert_eq!(editor.value("step-name"), "Unsaved local name");
  assert_eq!(editor.value("step-budget"), "invalid budget");
  assert_eq!(stored(&editor, id), before);
}

#[test]
fn pipelines_profile_link_and_leave_confirm_wait_for_actual_save_completion() {
  let (mut editor, id, assigned, _) = assigned_profile();
  open_step(&mut editor, id);
  // Another editor publishes the assigned profile after this inspector loaded its draft-only snapshot.
  editor
    .db()
    .publish_draft::<AgentProfile>(assigned, Timestamp::now())
    .expect("normal current profile publication makes this save body runnable");
  editor.set("step-name", "Saved before navigation");
  editor.click("pipeline-save");
  // Hold future delivery, rather than guessing how long the database write takes.
  editor.click("step-open-profile");
  stays(&editor, id);
  assert!(
    !editor.exists("pipeline-leave-question"),
    "the busy link has no departure handler"
  );
  editor.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Agent(assigned, Default::default()),
  ));
  editor.window.pass();
  assert!(editor.exists("pipeline-leave-question"));
  editor.click("pipeline-leave-confirm");
  stays(&editor, id);
  editor.click("pipeline-leave-keep");
  editor.until("actual pipeline save accepted", |e| !e.exists("pipeline-save"));
  assert_eq!(
    stored(&editor, id).body().steps()[0].name.as_str(),
    "Saved before navigation"
  );
  assert!(
    !editor.exists("step-open-profile"),
    "the accepted read now knows the profile is published"
  );
  editor.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Agent(assigned, Default::default()),
  ));
  current_agent(&mut editor, assigned, "Explorer");
}

#[test]
fn pipelines_profile_link_cannot_answer_or_leave_pending_gate_confirmation() {
  let (mut editor, id, ..) = assigned_profile();
  let before = stored(&editor, id);
  open_step(&mut editor, id);
  editor.click("step-gate");
  editor.click("pipeline-save");
  assert!(editor.exists("pipeline-save-question-confirm"));
  assert!(editor.gated());
  editor.click("step-open-profile");
  stays(&editor, id);
  assert!(editor.exists("pipeline-save-question-confirm"));
  assert!(!editor.exists("pipeline-leave-question"));
  assert_eq!(stored(&editor, id), before, "the protected question remains unanswered");
}

#[test]
fn pipelines_profile_link_waits_for_successful_reload_then_opens_current_agent() {
  let (mut editor, id, assigned, other) = assigned_profile();
  open_step(&mut editor, id);
  editor
    .db()
    .publish_draft::<AgentProfile>(assigned, Timestamp::now())
    .expect("normal external publication permits the old assignment to save");
  editor.set("step-name", "Local unsaved text");
  let before = stored(&editor, id);
  let mut edit = orchester_core::resource::PipelineEdit::new(before.body().clone());
  edit.set_description("External successful reload");
  edit.update(&StepKey::new("research").expect("fixture step key"), |step| {
    if let StepKind::Agent { profile, .. } = &mut step.kind {
      *profile = Some(other);
    }
  });
  let external = edit
    .validate()
    .expect("normal external draft assignment is structurally valid");
  editor
    .db()
    .save_draft(id, &Draft::new(external.clone(), before.based_on(), Timestamp::now()))
    .expect("normal external draft update persists");
  editor.click("pipeline-save");
  editor.until("stale save refused", |e| e.exists("pipeline-reload"));
  editor.click("pipeline-reload");
  assert!(editor.exists("pipeline-loading"));
  editor.click("step-open-profile");
  stays(&editor, id);
  assert!(!editor.exists("pipeline-leave-question"));
  editor.until("successful reload accepted", |e| !e.exists("pipeline-loading"));
  assert_eq!(stored(&editor, id).body(), &external);
  editor.click("pipeline-step-research");
  assert!(editor.text("step-open-profile").contains("Open Another profile"));
  editor.click("step-open-profile");
  current_agent(&mut editor, other, "Another profile");
}
