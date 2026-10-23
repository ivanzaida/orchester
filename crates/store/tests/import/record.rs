//! The import record: what each import copied, from where, and what it reused or left unresolved.

use orchester_core::{
  id::IdempotencyKey,
  import::{Catalog, Choice, Choices, Entity, Pick, Plan, Resolved},
  resource::{AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, Slug, VersionNumber},
  store::{ImportStore, ResourceStore},
};

use super::{at, batch, catalog_of, codex, name, personal_website, repository};

#[test]
fn import_record_keeps_every_copy_and_what_was_left_unresolved() {
  let (source_repo, preview) = personal_website();
  let source = catalog_of(source_repo.path());
  let (_target_repo, mut target) = repository("Kontur", "KON");

  let created = target
    .import(&batch(&source, &target, preview, None))
    .expect("imported");

  let record = target
    .import_record(created[1].id())
    .expect("read")
    .expect("the profile was imported");
  assert_eq!(record.imported_at, at(10));
  let copies: Vec<_> = record
    .copies
    .iter()
    .map(|(id, provenance)| (*id, provenance.resource_name.as_str(), provenance.version))
    .collect();
  assert_eq!(
    copies,
    [
      (created[0].id(), "Preview release", Some(VersionNumber::FIRST)),
      (created[1].id(), "Builder", None),
      (created[2].id(), "Release rules", None),
    ]
  );
  let provenance = record.provenance(created[0].id()).expect("the pipeline's");
  assert_eq!(provenance.project, source.project().id());
  assert_eq!(provenance.project_name.as_str(), "Personal website");
  assert_eq!(provenance.resource, preview);
  let codex = source.connections()[0].id();
  assert_eq!(
    record.resolved,
    [Resolved {
      source: Entity::Connection(codex),
      source_name: "Codex".to_owned(),
      target: None,
    }]
  );
  let made_here = target
    .create_resource(
      Slug::new("local").expect("slug"),
      name("Local"),
      &Pipeline::new(Vec::new()).expect("pipeline"),
      at(11),
    )
    .expect("created")
    .id();
  assert_eq!(target.import_record(made_here).expect("read"), None);
}

#[test]
fn import_record_keeps_the_source_name_of_a_renamed_copy_and_what_was_reused() {
  let (source_repo, preview) = personal_website();
  let source = catalog_of(source_repo.path());
  let (_target_repo, mut target) = repository("Kontur", "KON");
  let connection = codex(&mut target);
  let builder = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build here.").expect("prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  let own = target
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &builder, at(2))
    .expect("profile")
    .id();
  let target_catalog = Catalog::read(target.project().clone(), &target).expect("the target reads");
  let mut choices = Choices::default();
  choices.choose(Entity::Resource(preview), Choice::Rename);
  choices.rename(Entity::Resource(preview), "Website preview".to_owned());
  let plan = Plan::new(
    &source,
    &target_catalog,
    &[(preview, Pick::Published(VersionNumber::FIRST))],
    &choices,
  );
  let batch = plan.batch(&source, IdempotencyKey::generate(), at(10)).expect("ready");

  let created = target.import(&batch).expect("imported");

  assert_eq!(created[0].name().as_str(), "Website preview");
  let record = target
    .import_record(created[0].id())
    .expect("read")
    .expect("the pipeline was imported");
  let provenance = record.provenance(created[0].id()).expect("the pipeline's");
  assert_eq!(provenance.resource_name.as_str(), "Preview release");
  let builder_source = source
    .resources()
    .iter()
    .find(|entry| entry.resource().name().as_str() == "Builder")
    .expect("Builder")
    .resource()
    .id();
  assert_eq!(
    record.resolved,
    [Resolved {
      source: Entity::Resource(builder_source),
      source_name: "Builder".to_owned(),
      target: Some(Entity::Resource(own)),
    }]
  );
}
