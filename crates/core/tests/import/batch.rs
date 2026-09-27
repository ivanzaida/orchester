use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  import::{Choice, Choices, Content, CopiedResource, Entity, ImportBatch, ImportError, Pick, Plan},
  resource::{AllowedSteps, HttpAuth, InstructionScope, StepKind, ToolAction},
  time::Timestamp,
};

use super::fixture::{Source, Target, codex, draft, profile, source, target, version};

fn copy_of(batch: &ImportBatch, source: ResourceId) -> &CopiedResource {
  batch
    .copies()
    .iter()
    .find(|copy| copy.provenance().resource == source)
    .unwrap_or_else(|| panic!("a copy of {source}"))
}

/// Preview release v3 into a Kontur that has its own Builder: Builder is reused, Deploy preview
/// is copied as Website preview and its token is Kontur's own.
fn preview_release(source: &Source, target: &Target) -> ImportBatch {
  let mut choices = Choices::default();
  let deploy = Entity::Resource(source.deploy);
  choices.choose(deploy, Choice::Rename);
  choices.rename(deploy, "Website preview".to_owned());
  choices.choose(Entity::Secret(source.key), Choice::Use(Entity::Secret(target.key)));
  let selection = [(source.pipeline, Pick::Published(version(3)))];
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);
  plan
    .batch(
      &source.catalog,
      IdempotencyKey::generate(),
      Timestamp::from_unix_millis(9),
    )
    .expect("the plan is ready")
}

fn kontur_with_builder() -> (Target, ResourceId) {
  let own = codex("Codex");
  let target = target(vec![draft("builder", "Builder", profile("Builder", own.id()))]);
  let builder = target.catalog.resources()[0].resource().id();
  (target, builder)
}

#[test]
fn import_batch_points_copies_at_copies_and_at_the_target_own() {
  let source = source();
  let (target, builder) = kontur_with_builder();

  let batch = preview_release(&source, &target);

  assert_eq!(
    batch.copies().len(),
    3,
    "Preview release, Website preview and Release rules"
  );
  let pipeline = copy_of(&batch, source.pipeline);
  let deploy = copy_of(&batch, source.deploy);
  let rules = copy_of(&batch, source.rules);
  for (copy, original) in [
    (pipeline, source.pipeline),
    (deploy, source.deploy),
    (rules, source.rules),
  ] {
    assert_ne!(copy.resource().id(), original, "every copy has a new id");
  }
  let Content::Pipeline(steps) = pipeline.content() else {
    panic!("a pipeline");
  };
  let executors: Vec<_> = steps.steps().iter().map(|step| step.kind.executor()).collect();
  assert_eq!(executors, [Some(builder), Some(deploy.resource().id())]);
  assert!(matches!(steps.steps()[0].kind, StepKind::Agent { .. }));
  assert_eq!(deploy.resource().name().as_str(), "Website preview");
  assert_eq!(deploy.resource().slug().as_str(), "website-preview");
  let Content::Instruction(instruction) = rules.content() else {
    panic!("an instruction");
  };
  assert_eq!(
    instruction.scope(),
    &InstructionScope::Pipeline {
      pipeline: pipeline.resource().id()
    }
  );
  assert!(batch.relies_on().contains(&Entity::Resource(builder)));
  assert!(batch.relies_on().contains(&Entity::Secret(target.key)));
}

#[test]
fn import_batch_keeps_only_the_access_of_copied_pipelines() {
  let source = source();
  let (target, _) = kontur_with_builder();

  let batch = preview_release(&source, &target);

  let Content::Tool(tool) = copy_of(&batch, source.deploy).content() else {
    panic!("a tool");
  };
  let ToolAction::Http(request) = tool.action() else {
    panic!("an HTTP tool");
  };
  assert_eq!(request.auth(), HttpAuth::Bearer { secret: target.key });
  let callers: Vec<_> = tool
    .access()
    .expect("an access policy")
    .callers()
    .iter()
    .map(|(caller, steps)| (*caller, steps.clone()))
    .collect();
  assert_eq!(
    callers,
    [(copy_of(&batch, source.pipeline).resource().id(), AllowedSteps::All)],
    "the copy of Preview release may call it; Nightly was not imported, so it lost access"
  );
}

#[test]
fn import_batch_records_where_each_copy_came_from() {
  let source = source();
  let (target, _) = kontur_with_builder();

  let batch = preview_release(&source, &target);

  let provenance = copy_of(&batch, source.pipeline).provenance();
  assert_eq!(provenance.project, source.catalog.project().id());
  assert_eq!(provenance.project_name.as_str(), "Personal website");
  assert_eq!(provenance.version, Some(version(3)));
  assert!(
    batch
      .copies()
      .iter()
      .all(|copy| copy.resource().created_at() == Timestamp::from_unix_millis(9))
  );
}

#[test]
fn import_batch_copies_a_draft_when_the_draft_is_picked() {
  let source = source();
  let target = target(Vec::new());
  let selection = [(source.pipeline, Pick::Draft)];
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  let batch = plan
    .batch(
      &source.catalog,
      IdempotencyKey::generate(),
      Timestamp::from_unix_millis(9),
    )
    .expect("ready");

  let pipeline = copy_of(&batch, source.pipeline);
  let Content::Pipeline(body) = pipeline.content() else {
    panic!("a pipeline");
  };
  assert_eq!(body.description(), "Draft");
  assert_eq!(pipeline.provenance().version, None);
}

#[test]
fn import_batch_leaves_an_unresolved_connection_for_the_draft_to_show() {
  let source = source();
  let target = target(Vec::new());
  let selection = [(source.builder, Pick::Published(version(2)))];
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  let batch = plan
    .batch(
      &source.catalog,
      IdempotencyKey::generate(),
      Timestamp::from_unix_millis(9),
    )
    .expect("an unresolved connection does not keep the import from starting");

  let Content::AgentProfile(profile) = copy_of(&batch, source.builder).content() else {
    panic!("a profile");
  };
  assert_eq!(profile.connection(), source.codex);
  assert!(
    target.catalog.connection(profile.connection()).is_none(),
    "Kontur has no such connection: the copy shows it as missing"
  );
  assert!(batch.relies_on().is_empty());
}

#[test]
fn import_batch_refuses_a_plan_with_a_problem() {
  let source = source();
  let own = codex("Codex");
  let target = target(vec![draft("builder-2", "Builder", profile("Builder", own.id()))]);
  let selection = [(source.builder, Pick::Published(version(2)))];
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  let refused = plan.batch(
    &source.catalog,
    IdempotencyKey::generate(),
    Timestamp::from_unix_millis(9),
  );

  assert_eq!(refused, Err(ImportError::NotReady));
}
