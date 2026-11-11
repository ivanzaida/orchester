//! The import record: what each import copied, from where, and what it reused or left unresolved.

use orchester_core::{
  id::IdempotencyKey,
  import::{Catalog, Choice, Choices, Entity, Pick, Plan, Resolved},
  resource::{AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, Slug},
  store::{ImportStore, ResourceStore},
};

use super::{at, batch, builder_of, catalog_of, codex, name, personal_website, repository};

#[test]
fn import_record_keeps_every_copy_and_what_was_left_unresolved() {
  let (source_repo, _) = personal_website();
  let source = catalog_of(source_repo.path());
  let (_target_repo, mut target) = repository("Kontur", "KON");

  let created = target.import(&batch(&source, &target, None)).expect("imported");

  let record = target
    .import_record(created[0].id())
    .expect("read")
    .expect("the profile was imported");
  assert_eq!(record.imported_at, at(10));
  let copies: Vec<_> = record
    .copies
    .iter()
    .map(|(id, provenance)| (*id, provenance.resource_name.as_str(), provenance.version))
    .collect();
  assert_eq!(copies, [(created[0].id(), "Builder", None)]);
  let provenance = record.provenance(created[0].id()).expect("the profile's");
  assert_eq!(provenance.project, source.project().id());
  assert_eq!(provenance.project_name.as_str(), "Personal website");
  assert_eq!(provenance.resource, builder_of(&source));
  let codex = source.connections()[0].id();
  assert_eq!(
    record.resolved,
    [Resolved {
      source: Entity::Connection(codex),
      source_name: "Codex".to_owned(),
      target: None,
    }]
  );
  let steps = orchester_store::ProjectDb::open(source_repo.path())
    .expect("the source")
    .latest_version::<Pipeline>(preview)
    .expect("read")
    .expect("published")
    .body()
    .clone();
  // A pipeline is created as its first version (D-032).
  let (made_here, _) = target
    .published_pipeline_for_test("local", "Local", &steps, at(11))
    .expect("created");
  assert_eq!(target.import_record(made_here).expect("read"), None);
}

#[test]
fn import_record_keeps_the_source_name_of_a_renamed_copy_and_what_was_reused() {
  let (source_repo, _) = personal_website();
  let source = catalog_of(source_repo.path());
  let builder_source = builder_of(&source);
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
  target
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &builder, at(2))
    .expect("profile");
  let target_catalog = Catalog::read(target.project().clone(), &target).expect("the target reads");
  let mut choices = Choices::default();
  choices.choose(Entity::Resource(builder_source), Choice::Rename);
  choices.rename(Entity::Resource(builder_source), "Website builder".to_owned());
  choices.choose(
    Entity::Connection(source.connections()[0].id()),
    Choice::Use(Entity::Connection(connection)),
  );
  let plan = Plan::new(&source, &target_catalog, &[(builder_source, Pick::Draft)], &choices);
  let batch = plan.batch(&source, IdempotencyKey::generate(), at(10)).expect("ready");

  let created = target.import(&batch).expect("imported");

  assert_eq!(created[0].name().as_str(), "Website builder");
  let record = target
    .import_record(created[0].id())
    .expect("read")
    .expect("the profile was imported");
  let provenance = record.provenance(created[0].id()).expect("the profile's");
  assert_eq!(provenance.resource_name.as_str(), "Builder");
  assert_eq!(
    record.resolved,
    [Resolved {
      source: Entity::Connection(source.connections()[0].id()),
      source_name: "Codex".to_owned(),
      target: Some(Entity::Connection(connection)),
    }]
  );
}
