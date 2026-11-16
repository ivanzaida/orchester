//! A pipeline is stored only as a published version (D-032): it is created as v1, and an edit of it is saved by
//! publishing it as the next version, renamed when asked, in one transaction that keeps no draft and changes nothing
//! when it is refused.
//! `cargo test -p orchester-store --test resources_pipeline_edits resources_pipeline`.

use std::{
  collections::{BTreeSet, HashMap},
  fs,
};

use orchester_core::{
  id::{ProjectId, ResourceId, ServerId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    Draft, Pipeline, PromptText, PublishError, RepositoryAccess, ResourceKind, Step, StepKey, StepKind, VersionNumber,
  },
  sharing::MemberRole,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharingLink, StoreError};
use tempfile::TempDir;

fn project_repo() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    name("Kontur"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

/// A team project in which the person is an owner: what they publish waits for the server.
fn team_repo() -> (TempDir, ProjectDb) {
  let (repo, mut db) = project_repo();
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("ORC").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: at(1),
    cursor: 0,
    last_synced_at: at(1),
    removed_at: None,
    address: Some("https://team.example/orchester".to_owned()),
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("shared");
  (repo, db)
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn step(key: &str, profile: Option<ResourceId>) -> Step {
  Step {
    key: StepKey::new(key).expect("key"),
    name: name(key),
    kind: StepKind::Agent {
      profile,
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Say hello.").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

/// A pipeline whose steps `keys` each run an agent profile.
fn pipeline(keys: &[&str]) -> Pipeline {
  let profile = ResourceId::generate();
  Pipeline::new(keys.iter().map(|key| step(key, Some(profile))).collect()).expect("a pipeline")
}

/// Delivery, created as v1 of one step.
fn delivery(db: &mut ProjectDb) -> ResourceId {
  let (id, v1) = db
    .published_pipeline_for_test("delivery", "Delivery", &pipeline(&["implement"]), at(1))
    .expect("created");
  assert_eq!(v1.number(), VersionNumber::FIRST);
  id
}

fn versions(db: &ProjectDb, id: ResourceId) -> usize {
  db.versions::<Pipeline>(id).expect("versions").len()
}

fn title(db: &ProjectDb, id: ResourceId) -> String {
  db.resource(id)
    .expect("read")
    .expect("the pipeline")
    .name()
    .as_str()
    .to_owned()
}

#[test]
fn resources_pipeline_is_created_as_its_first_version_with_no_draft() {
  let (_repo, mut db) = project_repo();
  let id = delivery(&mut db);

  assert_eq!(versions(&db, id), 1);
  assert!(db.draft::<Pipeline>(id).expect("draft").is_none());
  assert_eq!(db.resources(ResourceKind::Pipeline).expect("listed").len(), 1);
}

#[test]
fn resources_pipeline_is_neither_created_as_a_draft_nor_given_one() {
  let (repo, mut db) = project_repo();
  let created = db.create_resource(
    orchester_core::resource::Slug::new("sketch").expect("slug"),
    name("Sketch"),
    &pipeline(&["implement"]),
    at(1),
  );
  assert!(
    matches!(created, Err(StoreError::NoDrafts(ResourceKind::Pipeline))),
    "{created:?}"
  );
  assert!(db.resources(ResourceKind::Pipeline).expect("listed").is_empty());

  let bare = ResourceId::generate();
  rusqlite::Connection::open(repo.path().join(".orchester/orchester.db"))
    .expect("open raw database")
    .execute(
      "INSERT INTO resources (id, kind, slug, name, created_at) VALUES (?1, 'pipeline', 'bare', 'Bare', 1)",
      [bare.to_string()],
    )
    .expect("a pipeline without version or draft");
  let published = delivery(&mut db);
  for (id, based_on) in [(bare, None), (published, Some(VersionNumber::FIRST))] {
    let refused = db.save_draft(id, &Draft::new(pipeline(&["plan"]), based_on, at(2)));
    assert!(
      matches!(refused, Err(StoreError::NoDrafts(ResourceKind::Pipeline))),
      "{refused:?}"
    );
    assert!(db.draft::<Pipeline>(id).expect("draft").is_none());
  }
}

#[test]
fn resources_pipeline_edit_publishes_the_next_version_renamed_and_keeps_no_draft() {
  let (_repo, mut db) = project_repo();
  let id = delivery(&mut db);
  let edited = pipeline(&["implement", "review"]);

  let v2 = db
    .publish_pipeline_edit(
      id,
      &edited,
      VersionNumber::FIRST,
      Some(("Delivery", &name("Release"))),
      at(2),
    )
    .expect("published");

  assert_eq!(v2.number().get(), 2);
  assert_eq!(v2.body(), &edited);
  assert_eq!(db.latest_version::<Pipeline>(id).expect("latest"), Some(v2));
  assert!(db.draft::<Pipeline>(id).expect("draft").is_none());
  assert_eq!(title(&db, id), "Release");
}

#[test]
fn resources_pipeline_edit_without_a_rename_keeps_the_name() {
  let (_repo, mut db) = project_repo();
  let id = delivery(&mut db);

  db.publish_pipeline_edit(id, &pipeline(&["review"]), VersionNumber::FIRST, None, at(2))
    .expect("published");

  assert_eq!(versions(&db, id), 2);
  assert_eq!(title(&db, id), "Delivery");
}

#[test]
fn resources_pipeline_edit_from_an_older_version_is_stale_and_changes_nothing() {
  let (_repo, mut db) = project_repo();
  let id = delivery(&mut db);
  db.publish_pipeline_edit(id, &pipeline(&["review"]), VersionNumber::FIRST, None, at(2))
    .expect("v2 by another editor");

  let refused = db.publish_pipeline_edit(
    id,
    &pipeline(&["plan"]),
    VersionNumber::FIRST,
    Some(("Delivery", &name("Release"))),
    at(3),
  );

  assert!(
    matches!(
      &refused,
      Err(StoreError::Publish(PublishError::StaleDraft { based_on: Some(based_on), latest: Some(latest) }))
        if *based_on == VersionNumber::FIRST && latest.get() == 2
    ),
    "{refused:?}"
  );
  assert_eq!(versions(&db, id), 2);
  assert_eq!(title(&db, id), "Delivery", "the rename went with the refused edit");
}

#[test]
fn resources_pipeline_edit_whose_name_changed_meanwhile_changes_nothing() {
  let (_repo, mut db) = project_repo();
  let id = delivery(&mut db);

  let refused = db.publish_pipeline_edit(
    id,
    &pipeline(&["review"]),
    VersionNumber::FIRST,
    Some(("Not its name", &name("Release"))),
    at(2),
  );

  assert!(
    matches!(refused, Err(StoreError::DraftChanged(changed)) if changed == id),
    "{refused:?}"
  );
  assert_eq!(versions(&db, id), 1);
  assert_eq!(title(&db, id), "Delivery");
}

#[test]
fn resources_pipeline_edit_that_cannot_be_published_changes_nothing() {
  let (_repo, mut db) = project_repo();
  let id = delivery(&mut db);
  let unassigned = Pipeline::new(vec![step("implement", None)]).expect("a pipeline");

  let refused = db.publish_pipeline_edit(
    id,
    &unassigned,
    VersionNumber::FIRST,
    Some(("Delivery", &name("Release"))),
    at(2),
  );

  assert!(
    matches!(&refused, Err(StoreError::Publish(PublishError::UnassignedSteps(steps))) if steps.len() == 1),
    "{refused:?}"
  );
  assert_eq!(versions(&db, id), 1);
  assert!(
    db.draft::<Pipeline>(id).expect("draft").is_none(),
    "nothing is kept as a draft"
  );
  assert_eq!(title(&db, id), "Delivery");
}

#[test]
fn resources_pipeline_edit_drops_a_draft_saved_before_pipelines_had_none() {
  let (repo, mut db) = project_repo();
  let id = delivery(&mut db);
  let body: String = rusqlite::Connection::open(repo.path().join(".orchester/orchester.db"))
    .expect("open raw database")
    .query_row(
      "SELECT body FROM resource_versions WHERE resource_id = ?1",
      [id.to_string()],
      |row| row.get(0),
    )
    .expect("v1's body");
  // A draft an older build saved over v1.
  rusqlite::Connection::open(repo.path().join(".orchester/orchester.db"))
    .expect("open raw database")
    .execute(
      "INSERT INTO resource_drafts (resource_id, body, based_on, updated_at) VALUES (?1, ?2, 1, 2)",
      [id.to_string(), body],
    )
    .expect("a draft over v1");
  assert!(db.draft::<Pipeline>(id).expect("draft").is_some());

  db.publish_pipeline_edit(id, &pipeline(&["review"]), VersionNumber::FIRST, None, at(3))
    .expect("published");

  assert!(db.draft::<Pipeline>(id).expect("draft").is_none());
}

#[test]
fn resources_pipeline_edit_of_another_kind_is_refused() {
  let (_repo, mut db) = project_repo();
  let id = delivery(&mut db);
  let rule = orchester_core::resource::Instruction::new(
    PromptText::new("Be brief.").expect("prompt"),
    orchester_core::resource::AgentTargets::All,
    BTreeSet::from([orchester_core::resource::InjectionEvent::Start]),
    orchester_core::resource::InstructionScope::Project,
  )
  .expect("an instruction");
  let (instruction, _) = db
    .create_published(
      orchester_core::resource::Slug::new("rule").expect("slug"),
      name("Rule"),
      &rule,
      at(2),
    )
    .expect("an instruction");

  let refused = db.publish_pipeline_edit(
    instruction.id(),
    &pipeline(&["review"]),
    VersionNumber::FIRST,
    None,
    at(3),
  );

  assert!(
    matches!(refused, Err(StoreError::ResourceKindMismatch { .. })),
    "{refused:?}"
  );
  assert_eq!(versions(&db, id), 1);
}

#[test]
fn resources_pipeline_edit_of_a_team_pipeline_is_refused_here() {
  let (_repo, mut db) = team_repo();
  let id = delivery(&mut db);

  // Only the team server takes a team pipeline's edit (D-043): nothing is published here.
  let refused = db.publish_pipeline_edit(id, &pipeline(&["review"]), VersionNumber::FIRST, None, at(2));
  assert!(matches!(refused, Err(StoreError::SharedProject)), "{refused:?}");
  assert_eq!(versions(&db, id), 1);

  db.record_sharing_state(Some(MemberRole::Member), Some(at(3)), None)
    .expect("now a member");
  let refused = db.publish_pipeline_edit(
    id,
    &pipeline(&["plan"]),
    VersionNumber::FIRST,
    Some(("Delivery", &name("Release"))),
    at(4),
  );

  assert!(matches!(refused, Err(StoreError::NotAdmin)), "{refused:?}");
  assert_eq!(versions(&db, id), 1);
  assert_eq!(title(&db, id), "Delivery");
}
