use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::Path,
};

use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, AllowedSteps, ApprovalMode, CommandAction, DeniedBehavior,
    Draft, InjectionEvent, Instruction, InstructionScope, ModelId, Pipeline, PromptText, ProviderSettings,
    PublishError, RepositoryAccess, ResourceKind, Slug, Step, StepKey, StepKind, Tool, ToolAction, VersionNumber,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection;
use tempfile::TempDir;

fn project_repo() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn raw(repo: &Path) -> Connection {
  Connection::open(repo.join(".orchester/orchester.db")).expect("open raw database")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn slug(value: &str) -> Slug {
  Slug::new(value).expect("valid slug")
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn instruction(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("valid prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("valid instruction")
}

fn profile() -> AgentProfile {
  AgentProfile::new(
    name("Reviewer"),
    "Reviews changes independently.".into(),
    PromptText::new("Review {{run.revision}}.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("model-a").expect("valid model"),
    ProviderSettings::default(),
  )
}

fn pipeline() -> Pipeline {
  let step = |key: &str, depends_on: &[&str]| Step {
    key: StepKey::new(key).expect("valid key"),
    name: name(key),
    kind: StepKind::Agent {
      profile: ResourceId::generate(),
      repository: RepositoryAccess::Write,
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: depends_on
      .iter()
      .map(|key| StepKey::new(key).expect("valid key"))
      .collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  Pipeline::new(vec![step("plan", &[]), step("implement", &["plan"])]).expect("valid pipeline")
}

fn tool() -> Tool {
  let access = AccessPolicy::new(
    BTreeMap::from([(ResourceId::generate(), AllowedSteps::All)]),
    name("Staging"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("valid policy");
  let action = CommandAction::new("cargo".into(), vec!["test".into()]).expect("valid command");
  Tool::new(ToolAction::Command(action), access)
}

#[test]
fn resources_are_created_with_a_first_draft_and_listed_by_kind() {
  let (_repo, mut db) = project_repo();
  let rules = db
    .create_resource(
      slug("git-conventions"),
      name("Git conventions"),
      &instruction("Rule."),
      at(1),
    )
    .unwrap();
  let style = db
    .create_resource(
      slug("communication-style"),
      name("Communication style"),
      &instruction("Be brief."),
      at(2),
    )
    .unwrap();
  db.create_resource(slug("reviewer"), name("Reviewer"), &profile(), at(3))
    .unwrap();

  assert_eq!(rules.kind(), ResourceKind::Instruction);
  assert_eq!(db.resource(rules.id()).unwrap(), Some(rules.clone()));
  assert_eq!(db.resource(ResourceId::generate()).unwrap(), None);
  let listed = db.resources(ResourceKind::Instruction).unwrap();
  assert_eq!(listed, [style, rules.clone()]);
  assert_eq!(db.resources(ResourceKind::Pipeline).unwrap(), []);

  let draft = db.draft::<Instruction>(rules.id()).unwrap().unwrap();
  assert_eq!(draft.body(), &instruction("Rule."));
  assert_eq!(draft.based_on(), None);
  assert_eq!(draft.updated_at(), at(1));
  assert_eq!(db.latest_version::<Instruction>(rules.id()).unwrap(), None);
}

#[test]
fn resources_slugs_are_unique_per_kind() {
  let (_repo, mut db) = project_repo();
  db.create_resource(slug("deploy"), name("Deploy"), &tool(), at(1))
    .unwrap();
  assert!(matches!(
    db.create_resource(slug("deploy"), name("Deploy again"), &tool(), at(2)),
    Err(StoreError::DuplicateSlug {
      kind: ResourceKind::Tool,
      ..
    })
  ));
  db.create_resource(slug("deploy"), name("Deploy"), &pipeline(), at(3))
    .unwrap();
}

#[test]
fn resources_publish_in_sequence_and_keep_old_versions() {
  let (_repo, mut db) = project_repo();
  let id = db
    .create_resource(slug("feature-delivery"), name("Feature delivery"), &pipeline(), at(1))
    .unwrap()
    .id();
  let v1 = db.publish_draft::<Pipeline>(id, at(2)).unwrap();
  assert_eq!(v1.number(), VersionNumber::FIRST);
  assert_eq!(db.draft::<Pipeline>(id).unwrap(), None);
  assert!(matches!(
    db.publish_draft::<Pipeline>(id, at(3)),
    Err(StoreError::NoDraft(missing)) if missing == id
  ));

  let mut draft = v1.to_draft(at(4));
  let single = Pipeline::new(pipeline().steps()[..1].to_vec()).unwrap();
  draft.edit(single.clone(), at(5));
  db.save_draft(id, &draft).unwrap();
  assert_eq!(db.draft::<Pipeline>(id).unwrap(), Some(draft));
  let v2 = db.publish_draft::<Pipeline>(id, at(6)).unwrap();

  assert_eq!(v2.number().get(), 2);
  assert_eq!(db.latest_version::<Pipeline>(id).unwrap(), Some(v2.clone()));
  assert_eq!(db.version::<Pipeline>(id, VersionNumber::FIRST).unwrap(), Some(v1));
  assert_eq!(
    db.version::<Pipeline>(id, v2.number()).unwrap().unwrap().body(),
    &single
  );
}

#[test]
fn resources_reject_publishing_a_stale_draft() {
  let (_repo, mut db) = project_repo();
  let id = db
    .create_resource(slug("reviewer"), name("Reviewer"), &profile(), at(1))
    .unwrap()
    .id();
  let v1 = db.publish_draft::<AgentProfile>(id, at(2)).unwrap();
  db.save_draft(id, &v1.to_draft(at(3))).unwrap();
  let v2 = db.publish_draft::<AgentProfile>(id, at(4)).unwrap();

  let stale = v1.to_draft(at(5));
  db.save_draft(id, &stale).unwrap();
  assert!(matches!(
    db.publish_draft::<AgentProfile>(id, at(6)),
    Err(StoreError::Publish(PublishError::StaleDraft { based_on, latest }))
      if based_on == Some(v1.number()) && latest == Some(v2.number())
  ));
  assert_eq!(db.draft::<AgentProfile>(id).unwrap(), Some(stale));
  assert_eq!(db.latest_version::<AgentProfile>(id).unwrap(), Some(v2));
}

#[test]
fn resources_reject_the_wrong_kind_and_unknown_ids() {
  let (_repo, mut db) = project_repo();
  let id = db
    .create_resource(slug("deploy"), name("Deploy"), &tool(), at(1))
    .unwrap()
    .id();
  assert!(matches!(
    db.draft::<Pipeline>(id),
    Err(StoreError::ResourceKindMismatch {
      expected: ResourceKind::Pipeline,
      found: ResourceKind::Tool,
      ..
    })
  ));
  assert!(matches!(
    db.save_draft(id, &Draft::new(pipeline(), None, at(2))),
    Err(StoreError::ResourceKindMismatch { .. })
  ));
  let unknown = ResourceId::generate();
  assert!(matches!(
    db.publish_draft::<Tool>(unknown, at(3)),
    Err(StoreError::ResourceNotFound(missing)) if missing == unknown
  ));
}

#[test]
fn resources_persist_across_reopen() {
  let (repo, mut db) = project_repo();
  let resource = db
    .create_resource(slug("deploy"), name("Deploy preview"), &tool(), at(1))
    .unwrap();
  let v1 = db.publish_draft::<Tool>(resource.id(), at(2)).unwrap();
  let draft = v1.to_draft(at(3));
  db.save_draft(resource.id(), &draft).unwrap();
  drop(db);

  let db = ProjectDb::open(repo.path()).unwrap();
  assert_eq!(db.resource(resource.id()).unwrap(), Some(resource.clone()));
  assert_eq!(db.latest_version::<Tool>(resource.id()).unwrap(), Some(v1));
  assert_eq!(db.draft::<Tool>(resource.id()).unwrap(), Some(draft));
}

#[test]
fn resources_revalidate_stored_bodies_on_read() {
  let (repo, mut db) = project_repo();
  let id = db
    .create_resource(slug("feature-delivery"), name("Feature delivery"), &pipeline(), at(1))
    .unwrap()
    .id();
  let stored: String = raw(repo.path())
    .query_row("SELECT body FROM resource_drafts", [], |row| row.get(0))
    .unwrap();
  assert!(stored.contains(r#""schema":1"#), "{stored}");

  let cyclic = stored.replace(r#""depends_on":[]"#, r#""depends_on":["implement"]"#);
  let newer = stored.replace(r#""schema":1"#, r#""schema":2"#);
  for body in [cyclic.as_str(), newer.as_str(), "not json"] {
    raw(repo.path())
      .execute("UPDATE resource_drafts SET body = ?1", [body])
      .unwrap();
    assert!(
      matches!(db.draft::<Pipeline>(id), Err(StoreError::Corrupt { .. })),
      "{body}"
    );
    assert!(matches!(
      db.publish_draft::<Pipeline>(id, at(2)),
      Err(StoreError::Corrupt { .. })
    ));
  }
}

#[test]
fn resources_published_versions_are_immutable() {
  let (repo, mut db) = project_repo();
  let id = db
    .create_resource(slug("rules"), name("Rules"), &instruction("Rule."), at(1))
    .unwrap()
    .id();
  db.publish_draft::<Instruction>(id, at(2)).unwrap();
  let raw = raw(repo.path());
  assert!(raw.execute("UPDATE resource_versions SET body = '{}'", []).is_err());
  assert!(raw.execute("DELETE FROM resource_versions", []).is_err());
  assert!(db.latest_version::<Instruction>(id).unwrap().is_some());
}
