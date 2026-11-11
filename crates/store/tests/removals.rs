//! Removing a resource (ORC-107): an unused one goes after a plain question; one a pipeline step runs is listed with
//! its users and stays, and only the latest version of a pipeline counts. Discard draft drops only the draft the
//! editor read.
use std::fs;

use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, Draft, ModelId, Pipeline, PromptText, ProviderSettings, Removal, RepositoryAccess, ResourceKind,
    Slug, Step, StepKey, StepKind,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{DraftChange, ProjectDb, StoreError};
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
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("valid key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(repo.path(), project).expect("create project");
    let builder = db
      .create_resource(
        Slug::new("builder").expect("slug"),
        name("Builder"),
        &profile(""),
        at(1),
      )
      .expect("create profile")
      .id();
    db.publish_draft::<AgentProfile>(builder, at(2))
      .expect("publish profile");
    Self {
      _repo: repo,
      db,
      builder,
    }
  }

  /// A published Delivery pipeline whose implement step runs Builder.
  fn pipeline(&mut self) -> ResourceId {
    self.pipeline_as("delivery", "Delivery")
  }

  /// A published pipeline `title` whose implement step runs Builder.
  fn pipeline_as(&mut self, slug: &str, title: &str) -> ResourceId {
    let pipeline = Pipeline::new(vec![step("implement", self.builder)]).expect("valid pipeline");
    let (id, _) = self
      .db
      .published_pipeline_for_test(slug, title, &pipeline, at(3))
      .expect("publish pipeline");
    id
  }

  /// The steps that run Builder, as the in-place question lists them.
  fn users(&mut self) -> Vec<orchester_core::resource::StepUser> {
    orchester_core::resource::step_users(&self.db, self.builder).expect("step users")
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
  let removal = fixture.db.remove_resource(fixture.builder, at(5)).expect("remove");
  assert_eq!(removal, Removal::Removed);
  assert!(!fixture.listed(), "a removed profile is not listed");
  let slugs = fixture.db.slugs(ResourceKind::AgentProfile).expect("slugs");
  assert!(
    slugs.iter().any(|slug| slug.as_str() == "builder"),
    "its slug stays taken"
  );
}

#[test]
fn removals_a_profile_in_use_lists_its_steps_and_stays() {
  let mut fixture = Fixture::new();
  let delivery = fixture.pipeline();
  let Removal::InUse(users) = fixture.db.remove_resource(fixture.builder, at(5)).expect("remove") else {
    panic!("a profile a step runs is in use");
  };
  assert_eq!(users.len(), 1);
  assert_eq!(users[0].pipeline, delivery);
  assert_eq!(users[0].number, 1);
  assert_eq!(users[0].version.get(), 1);
  assert!(fixture.listed(), "nothing is removed while it is in use");
}

#[test]
fn removals_only_the_latest_version_of_a_pipeline_counts() {
  let mut fixture = Fixture::new();
  let delivery = fixture.pipeline();
  let other = fixture
    .db
    .create_resource(Slug::new("other").expect("slug"), name("Other"), &profile(""), at(5))
    .expect("create profile")
    .id();
  fixture
    .db
    .publish_draft::<AgentProfile>(other, at(6))
    .expect("publish profile");
  let moved = Pipeline::new(vec![step("implement", other)]).expect("valid pipeline");
  let first = orchester_core::resource::VersionNumber::FIRST;
  fixture
    .db
    .publish_pipeline_edit(delivery, &moved, first, None, at(7))
    .expect("publish v2");
  assert_eq!(fixture.users(), [], "v1 ran Builder, v2 does not");
  let removal = fixture.db.remove_resource(fixture.builder, at(9)).expect("remove");
  assert_eq!(removal, Removal::Removed);
  assert!(!fixture.listed());
}

#[test]
fn removals_discard_draft_returns_to_the_published_version() {
  let mut fixture = Fixture::new();
  let changed = profile("A draft to throw away");
  let draft = Draft::new(
    changed.clone(),
    Some(orchester_core::resource::VersionNumber::FIRST),
    at(5),
  );
  fixture.db.save_draft(fixture.builder, &draft).expect("save draft");
  let other = profile("Not the draft the editor read");
  let refused = fixture.db.discard_draft_matching(fixture.builder, &other);
  assert!(matches!(refused, Err(StoreError::DraftChanged(_))), "{refused:?}");
  let version = fixture
    .db
    .discard_draft_matching(fixture.builder, &changed)
    .expect("discard");
  assert_eq!(version.get(), 1);
  assert!(
    fixture
      .db
      .draft::<AgentProfile>(fixture.builder)
      .expect("draft")
      .is_none()
  );
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

#[test]
fn removals_settle_draft_reads_and_writes_in_one_transaction() {
  let mut fixture = Fixture::new();
  let builder = fixture.builder;
  let first = orchester_core::resource::VersionNumber::FIRST;
  let typed = profile("Typed");

  // Nothing to settle: no draft and a body that is the version's own leaves nothing behind.
  fixture
    .db
    .settle_draft::<AgentProfile>(builder, |kept, latest| {
      assert!(kept.is_none());
      assert_eq!(latest.number(), first);
      Ok(DraftChange::Keep)
    })
    .expect("kept");
  assert!(fixture.db.draft::<AgentProfile>(builder).expect("draft").is_none());

  fixture
    .db
    .settle_draft::<AgentProfile>(builder, |_, _| {
      Ok(DraftChange::Replace(Draft::new(typed.clone(), Some(first), at(7))))
    })
    .expect("replaced");
  assert_eq!(
    fixture
      .db
      .draft::<AgentProfile>(builder)
      .expect("draft")
      .expect("kept")
      .body(),
    &typed
  );

  // A decision that fails writes nothing.
  let failed = fixture
    .db
    .settle_draft::<AgentProfile>(builder, |_, _| Err(StoreError::DraftChanged(builder)));
  assert!(matches!(failed, Err(StoreError::DraftChanged(_))), "{failed:?}");
  assert!(fixture.db.draft::<AgentProfile>(builder).expect("draft").is_some());

  // The draft the decision read is the one it discards.
  fixture
    .db
    .settle_draft::<AgentProfile>(builder, |kept, _| {
      assert_eq!(kept.map(Draft::body), Some(&typed));
      Ok(DraftChange::Discard)
    })
    .expect("discarded");
  assert!(fixture.db.draft::<AgentProfile>(builder).expect("draft").is_none());
}

#[test]
fn removals_settle_draft_of_a_resource_never_published_is_refused() {
  let mut fixture = Fixture::new();
  let reviewer = fixture
    .db
    .create_resource(
      Slug::new("reviewer").expect("slug"),
      name("Reviewer"),
      &profile(""),
      at(6),
    )
    .expect("create profile")
    .id();
  let refused = fixture
    .db
    .settle_draft::<AgentProfile>(reviewer, |_, _| Ok(DraftChange::Discard));
  assert!(matches!(refused, Err(StoreError::NotPublished(_))), "{refused:?}");
}
