//! Removing a resource (ORC-107): an unused one goes after a plain question; one a pipeline step runs is listed with
//! its users, and Remove and reset clears it from each of them. Discard draft drops only the draft the editor read.
use std::fs;

use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, Draft, ModelId, Pipeline, PromptText, ProviderSettings, RemoveMode, Removal, RepositoryAccess,
    ResourceKind, Slug, Step, StepKey, StepKind, StepVersion,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use tempfile::TempDir;

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn profile(responsibility: &str) -> AgentProfile {
  AgentProfile::new(
    name("Builder"),
    responsibility.into(),
    PromptText::new("Build.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("model-b").expect("valid model"),
    ProviderSettings::default(),
  )
}

fn step(key: &str, profile: ResourceId) -> Step {
  Step {
    key: StepKey::new(key).expect("valid key"),
    name: name(key),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

struct Fixture {
  _repo: TempDir,
  db: ProjectDb,
  builder: ResourceId,
}

impl Fixture {
  /// A project with a published Builder profile.
  fn new() -> Self {
    let repo = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(ProjectId::generate(), ProjectKey::new("ORC").expect("valid key"), name("Kontur"));
    let mut db = ProjectDb::create(repo.path(), project).expect("create project");
    let builder = db
      .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile(""), at(1))
      .expect("create profile")
      .id();
    db.publish_draft::<AgentProfile>(builder, at(2)).expect("publish profile");
    Self { _repo: repo, db, builder }
  }

  /// A published Delivery pipeline whose implement step runs Builder.
  fn pipeline(&mut self) -> ResourceId {
    let pipeline = Pipeline::new(vec![step("implement", self.builder)]).expect("valid pipeline");
    let delivery = self
      .db
      .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at(3))
      .expect("create pipeline")
      .id();
    self.db.publish_draft::<Pipeline>(delivery, at(4)).expect("publish pipeline");
    delivery
  }

  fn listed(&self) -> bool {
    self
      .db
      .resources(ResourceKind::AgentProfile)
      .expect("profiles")
      .iter()
      .any(|resource| resource.id() == self.builder)
  }
}

#[test]
fn removals_an_unused_profile_goes_and_keeps_its_slug() {
  let mut fixture = Fixture::new();
  let removal = fixture
    .db
    .remove_resource(fixture.builder, RemoveMode::Unused, at(5))
    .expect("remove");
  assert_eq!(removal, Removal::Removed);
  assert!(!fixture.listed(), "a removed profile is not listed");
  let slugs = fixture.db.slugs(ResourceKind::AgentProfile).expect("slugs");
  assert!(slugs.iter().any(|slug| slug.as_str() == "builder"), "its slug stays taken");
}

#[test]
fn removals_a_profile_in_use_lists_its_steps_and_stays() {
  let mut fixture = Fixture::new();
  let delivery = fixture.pipeline();
  let Removal::InUse(users) = fixture
    .db
    .remove_resource(fixture.builder, RemoveMode::Unused, at(5))
    .expect("remove")
  else {
    panic!("a profile a step runs is in use");
  };
  assert_eq!(users.len(), 1);
  assert_eq!(users[0].pipeline, delivery);
  assert_eq!(users[0].number, 1);
  assert!(matches!(users[0].version, StepVersion::Published(v) if v.get() == 1));
  assert!(fixture.listed(), "nothing is removed while it is in use");
}

#[test]
fn removals_remove_and_reset_clears_the_profile_from_its_steps() {
  let mut fixture = Fixture::new();
  let delivery = fixture.pipeline();
  let removal = fixture
    .db
    .remove_resource(fixture.builder, RemoveMode::Reset, at(5))
    .expect("remove and reset");
  assert_eq!(removal, Removal::Removed);
  assert!(!fixture.listed());
  let draft = fixture
    .db
    .draft::<Pipeline>(delivery)
    .expect("draft")
    .expect("the pipeline's draft asks to choose a profile");
  assert!(
    draft.body().steps_run_by(fixture.builder).next().is_none(),
    "no step of the draft runs the removed profile"
  );
}

#[test]
fn removals_discard_draft_returns_to_the_published_version() {
  let mut fixture = Fixture::new();
  let changed = profile("A draft to throw away");
  let draft = Draft::new(changed.clone(), Some(orchester_core::resource::VersionNumber::FIRST), at(5));
  fixture.db.save_draft(fixture.builder, &draft).expect("save draft");
  let other = profile("Not the draft the editor read");
  let refused = fixture.db.discard_draft_matching(fixture.builder, &other);
  assert!(matches!(refused, Err(StoreError::DraftChanged(_))), "{refused:?}");
  let version = fixture
    .db
    .discard_draft_matching(fixture.builder, &changed)
    .expect("discard");
  assert_eq!(version.get(), 1);
  assert!(fixture.db.draft::<AgentProfile>(fixture.builder).expect("draft").is_none());
}

#[test]
fn removals_a_draft_never_published_cannot_be_discarded() {
  let mut fixture = Fixture::new();
  let body = profile("");
  let reviewer = fixture
    .db
    .create_resource(Slug::new("reviewer").expect("slug"), name("Reviewer"), &body, at(6))
    .expect("create profile")
    .id();
  let refused = fixture.db.discard_draft_matching(reviewer, &body);
  assert!(matches!(refused, Err(StoreError::NotPublished(_))), "{refused:?}");
}
