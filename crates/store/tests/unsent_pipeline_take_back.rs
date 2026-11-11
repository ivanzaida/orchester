//! A pipeline's version published here that the team server has not numbered, as an older build left one waiting, is
//! never taken back as a draft: a pipeline holds no draft (D-032). A new build makes no such version: a team pipeline's
//! save waits for the team server (D-043).

use std::{collections::HashMap, fs, num::NonZeroU32};

use orchester_core::{
  blob::ContentHash,
  id::{ProjectId, ResourceId, ServerId, UserId, VersionId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{Pipeline, PipelineEdit, ResourceKind, Slug, StepKind, VersionNumber},
  sharing::{MemberRole, SharedPerson},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb, SharingLink, StoreError};
use tempfile::TempDir;

const HASH: &str = "5f1d6b0f0e2b9a8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c";

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn two() -> VersionNumber {
  VersionNumber::new(NonZeroU32::new(2).expect("two"))
}

/// A pipeline of one step running `tool`, described by `text`.
fn pipeline(tool: ResourceId, text: &str) -> Pipeline {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("an empty body"));
  let key = edit.add_step(Name::new("Check").expect("name"), None);
  edit.update(&key, |step| step.kind = StepKind::Tool { tool: Some(tool) });
  edit.set_description(text);
  edit.validate().expect("a valid pipeline")
}

/// `body` as the server numbered it `number`, published by olha.
fn cached(number: VersionNumber, body: &Pipeline) -> CachedVersion<Pipeline> {
  CachedVersion {
    number,
    published_at: at(3),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "olha".to_owned(),
    },
    server_body: serde_json::to_value(body).expect("a body"),
    hash: ContentHash::new(HASH).expect("hash"),
    references: Vec::new(),
    local_body: body.clone(),
    waits_here: false,
  }
}

/// A team project whose owner has the pipeline `Checks`, numbered v1 by the server, and its edit *Mine* published here
/// as v2, which waits to be sent: the pipeline and that version.
struct Team {
  _repo: TempDir,
  db: ProjectDb,
  pipeline: ResourceId,
  tool: ResourceId,
  mine: VersionId,
}

fn team_with_waiting_edit() -> Team {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    Name::new("Kontur").expect("name"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("project");
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
  let tool = ResourceId::generate();
  let (resource, first) = db
    .create_published(
      Slug::new("checks").expect("slug"),
      Name::new("Checks").expect("name"),
      &pipeline(tool, "Ours."),
      at(2),
    )
    .expect("created");
  db.confirm_publish::<Pipeline>(first.id(), &cached(VersionNumber::FIRST, &pipeline(tool, "Ours.")))
    .expect("numbered");
  let mine = db
    .publish_pipeline_edit(
      resource.id(),
      &pipeline(tool, "Mine."),
      VersionNumber::FIRST,
      None,
      at(4),
    )
    .expect("published here");
  Team {
    _repo: repo,
    db,
    pipeline: resource.id(),
    tool,
    mine: mine.id(),
  }
}

fn latest(team: &Team) -> (VersionNumber, Pipeline) {
  let version = team
    .db
    .latest_version::<Pipeline>(team.pipeline)
    .expect("read")
    .expect("a version");
  (version.number(), version.body().clone())
}

#[test]
fn a_pipeline_version_is_never_taken_back_as_a_draft() {
  let mut team = team_with_waiting_edit();

  let refused = team.db.reopen_draft::<Pipeline>(team.mine, at(6));

  assert!(
    matches!(refused, Err(StoreError::NoDrafts(ResourceKind::Pipeline))),
    "{refused:?}"
  );
  assert_eq!(
    team.db.waiting_publishes().expect("outbox").len(),
    1,
    "nothing is changed"
  );
  assert_eq!(latest(&team), (two(), pipeline(team.tool, "Mine.")));
}
