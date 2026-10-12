use orchester_core::{
  id::{ConnectionId, IdempotencyKey},
  name::Name,
  resource::{
    AgentProfile, InstructionScope, ModelId, Pipeline, PipelineEdit, PromptText, ProviderSettings, Slug, StepKind,
  },
  store::{ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};

use super::harness::Instructions;

pub(super) fn seed_preview(screen: &Instructions) -> orchester_core::id::ResourceId {
  let mut db = orchester_store::ProjectDb::open(&screen.root).expect("valid instruction fixture");
  let profile = AgentProfile::new(
    Name::new("Builder").expect("valid instruction fixture"),
    "Builds the change".into(),
    PromptText::new("Follow the plan.").expect("valid instruction fixture"),
    ConnectionId::generate(),
    ModelId::new("test").expect("valid instruction fixture"),
    ProviderSettings::default(),
  );
  let profile = db
    .create_resource(
      Slug::new("builder").expect("valid instruction fixture"),
      Name::new("Builder").expect("valid instruction fixture"),
      &profile,
      Timestamp::now(),
    )
    .expect("valid instruction fixture")
    .id();
  db.publish_draft::<AgentProfile>(profile, Timestamp::now())
    .expect("valid instruction fixture");
  let mut pipeline = PipelineEdit::new(Pipeline::new(Vec::new()).expect("valid instruction fixture"));
  let key = pipeline.add_step(Name::new("Implement").expect("valid instruction fixture"), None);
  pipeline.update(&key, |step| {
    if let StepKind::Agent { profile: chosen, .. } = &mut step.kind {
      *chosen = Some(profile);
    }
  });
  let body = pipeline.validate().expect("valid instruction fixture");
  let id = db
    .create_resource(
      Slug::new("delivery").expect("valid instruction fixture"),
      Name::new("Feature delivery").expect("valid instruction fixture"),
      &body,
      Timestamp::now(),
    )
    .expect("valid instruction fixture")
    .id();
  db.publish_draft::<Pipeline>(id, Timestamp::now())
    .expect("valid instruction fixture");
  db.create_tasks(
    IdempotencyKey::generate(),
    vec![TaskDetails {
      title: Name::new("Add retry to webhook sender").expect("valid instruction fixture"),
      brief: "Retry transient failures.".into(),
      acceptance_criteria: vec!["Retries 5xx responses".into()],
      pipeline: Some(id),
      limits: TaskLimits::NONE,
    }],
    Timestamp::now(),
  )
  .expect("valid instruction fixture");
  id
}
#[test]
fn instructions_preview_composes_real_sources_and_full_prompt_without_a_modal() {
  let mut screen = Instructions::new();
  let pipeline = seed_preview(&screen);
  screen.rule(
    "Standards",
    "Working on {{task.title}}",
    InstructionScope::Project,
    true,
  );
  screen.rule(
    "Pipeline rule",
    "Ship the change",
    InstructionScope::Pipeline { pipeline },
    true,
  );
  screen.go();
  screen.until("variables", |s| s.exists("instruction-variable-detail"));
  assert!(screen.text("instruction-variable-value").contains("Add retry"));
  screen.click("instruction-panel-1");
  screen.until("preview", |s| s.exists("instruction-full-prompt"));
  assert!(screen.text("instruction-panel").contains("Pipeline rule"));
  screen.click("instruction-full-prompt");
  screen.until("viewer", |s| s.exists("instruction-full-viewer"));
  let prompt = screen.text("instruction-full-viewer");
  assert!(prompt.contains("Working on Add retry to webhook sender"));
  assert!(!screen.exists("instruction-library"));
  screen.click("instruction-viewer-back");
  screen.until("editor", |s| s.exists("instruction-prompt"));
}
#[test]
fn instructions_missing_run_variable_is_flagged_and_copy_is_disabled() {
  let mut screen = Instructions::new();
  seed_preview(&screen);
  screen.rule(
    "Needs a checkpoint",
    "Compare {{run.revision}}",
    InstructionScope::Project,
    false,
  );
  screen.go();
  screen.click("instruction-panel-1");
  screen.until("missing variable", |s| s.exists("instruction-unavailable-variable"));
  assert!(screen.text("instruction-unavailable-variable").contains("run.revision"));
  screen.click("instruction-copy-prompt");
  screen.click("instruction-panel-0");
  screen.set("instruction-variable-search", "revision");
  screen.click("instruction-variable-group-run");
  screen.click("instruction-variable-run.revision");
  assert!(screen.text("instruction-variable-detail").contains("Unavailable"));
  screen.click("instruction-variable-resume");
  screen.pump();
  assert!(
    screen.listed().entries[0]
      .body
      .events()
      .contains(&orchester_core::resource::InjectionEvent::Resume)
  );
}
