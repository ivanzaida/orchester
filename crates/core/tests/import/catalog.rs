use orchester_core::{
  id::ResourceId,
  import::{CatalogResource, Entity, Pick},
  resource::{AgentTargets, InstructionScope, Resource, ResourceKind, Slug, StepKey},
  time::Timestamp,
};

use super::fixture::{instruction, name, source, version};

#[test]
fn import_catalog_lists_what_refers_to_a_resource() {
  let source = source();

  let users = |entity| -> Vec<&str> {
    source
      .catalog
      .users(entity)
      .map(|resource| resource.resource().name().as_str())
      .collect()
  };

  assert_eq!(users(Entity::Resource(source.builder)), ["Preview release", "Nightly"]);
  assert_eq!(
    users(Entity::Connection(source.codex)),
    ["Builder", "Reviewer"],
    "profiles refer to their connection"
  );
  assert_eq!(users(Entity::Resource(source.pipeline)), ["Release rules"]);
  assert!(users(Entity::Resource(source.rules)).is_empty());
}

#[test]
fn import_catalog_offers_the_published_version_first() {
  let source = source();
  let preview = source.catalog.resource(source.pipeline).expect("Preview release");

  assert_eq!(preview.default_pick(), Pick::Published(version(3)));
  assert_eq!(preview.picks(), [Pick::Published(version(3)), Pick::Draft]);
  assert!(
    preview.content(Pick::Published(version(2))).is_none(),
    "only the latest version is offered"
  );
}

#[test]
fn import_catalog_does_not_offer_pipelines_or_what_is_written_for_one() {
  let source = source();

  let offered = |id| source.catalog.resource(id).expect("in the catalog").is_offered();

  assert!(!offered(source.pipeline));
  assert!(!offered(source.nightly), "a pipeline that was never published too");
  assert!(!offered(source.rules), "an instruction for a pipeline");
  assert!(offered(source.builder) && offered(source.reviewer) && offered(source.deploy));
}

#[test]
fn import_catalog_does_not_offer_an_instruction_whose_draft_is_for_a_pipeline() {
  let source = source();
  let project_wide = instruction("Keep it short.", AgentTargets::All, InstructionScope::Project);
  let for_a_step = instruction(
    "Keep it short.",
    AgentTargets::All,
    InstructionScope::Step {
      pipeline: source.pipeline,
      step: StepKey::new("build").expect("key"),
    },
  );
  let resource = Resource::new(
    ResourceId::generate(),
    ResourceKind::Instruction,
    Slug::new("short").expect("slug"),
    name("Short"),
    Timestamp::from_unix_millis(1),
  );

  let both = CatalogResource::new(
    resource.clone(),
    Some((version(1), project_wide.clone())),
    Some(for_a_step.clone()),
  )
  .expect("content");
  let published_only = CatalogResource::new(resource.clone(), Some((version(1), project_wide)), None).expect("content");
  let step_only = CatalogResource::new(resource, None, Some(for_a_step)).expect("content");

  assert!(published_only.is_offered());
  assert!(!both.is_offered(), "its draft would bring the pipeline's id along");
  assert!(!step_only.is_offered());
}
