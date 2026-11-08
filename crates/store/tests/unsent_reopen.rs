//! A version published here that the team server has not numbered goes back to being the draft, whether it waits to be
//! sent or the server refused it (`ProjectDb::reopen_draft`, ORC-836): the hub's *Cancel sending* and a refused
//! *Publish for the team* both keep what the person wrote, edited from the version the server numbered last.

use std::{
  collections::{BTreeSet, HashMap},
  fs,
};

use orchester_core::{
  blob::ContentHash,
  id::{ProjectId, ResourceId, ServerId, UserId, VersionId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, Slug, VersionNumber},
  sharing::{MemberRole, SharedPerson, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb, PublishState, ResourceSharing, SharingLink};
use tempfile::TempDir;

const HASH: &str = "5f1d6b0f0e2b9a8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c";

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
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

/// A team project whose owner has the instruction `Rule`, numbered v1 by the server, and a version v2 published from a
/// draft that waits to be sent: the resource and that version.
fn team_with_waiting_edit() -> (TempDir, ProjectDb, ResourceId, VersionId) {
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
  let (rule, first) = db
    .create_published(
      Slug::new("rule").expect("slug"),
      Name::new("Rule").expect("name"),
      &instruction("One."),
      at(2),
    )
    .expect("created");
  let confirmed = CachedVersion {
    number: VersionNumber::FIRST,
    published_at: at(3),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "olha".to_owned(),
    },
    server_body: serde_json::to_value(instruction("One.")).expect("a body"),
    hash: ContentHash::new(HASH).expect("hash"),
    references: Vec::new(),
    local_body: instruction("One."),
    waits_here: false,
  };
  db.confirm_publish::<Instruction>(first.id(), &confirmed)
    .expect("numbered");
  db.save_draft(
    rule.id(),
    &Draft::new(instruction("Two."), Some(VersionNumber::FIRST), at(4)),
  )
  .expect("a draft");
  let edit = db
    .publish_draft::<Instruction>(rule.id(), at(5))
    .expect("published here");
  (repo, db, rule.id(), edit.id())
}

fn draft_text(db: &ProjectDb, rule: ResourceId) -> Option<Instruction> {
  db.draft::<Instruction>(rule)
    .expect("read")
    .map(|draft| draft.body().clone())
}

#[test]
fn a_version_that_waits_to_be_sent_goes_back_to_being_the_draft() {
  let (_repo, mut db, rule, edit) = team_with_waiting_edit();
  assert_eq!(db.waiting_publishes().expect("outbox").len(), 1);

  db.reopen_draft::<Instruction>(edit, at(6)).expect("taken back");

  assert!(db.waiting_publishes().expect("outbox").is_empty());
  assert_eq!(draft_text(&db, rule), Some(instruction("Two.")));
  let draft = db.draft::<Instruction>(rule).expect("read").expect("a draft");
  assert_eq!(
    draft.based_on(),
    Some(VersionNumber::FIRST),
    "edited from the version the server numbered"
  );
  assert_eq!(
    db.latest_version::<Instruction>(rule)
      .expect("read")
      .expect("a version")
      .number(),
    VersionNumber::FIRST,
    "nothing shows as published"
  );
}

#[test]
fn a_version_the_server_refused_goes_back_to_being_the_draft() {
  let (_repo, mut db, rule, edit) = team_with_waiting_edit();
  db.record_publish_refused(edit, "invalid:body").expect("recorded");
  assert!(matches!(
    db.waiting_publishes().expect("outbox")[0].state,
    PublishState::Refused { ref code } if code == "invalid:body"
  ));

  db.reopen_draft::<Instruction>(edit, at(6)).expect("taken back");

  assert!(db.waiting_publishes().expect("outbox").is_empty());
  assert_eq!(draft_text(&db, rule), Some(instruction("Two.")));
}

/// A teammate's v2 reaches this computer while the person's own v2 still waits (it takes number 3 meanwhile).
fn teammate_publishes(db: &mut ProjectDb, rule: ResourceId, text: &str) {
  let cached = CachedVersion {
    number: VersionNumber::new(std::num::NonZeroU32::new(2).expect("two")),
    published_at: at(7),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "olha".to_owned(),
    },
    server_body: serde_json::to_value(instruction(text)).expect("a body"),
    hash: ContentHash::new(HASH).expect("hash"),
    references: Vec::new(),
    local_body: instruction(text),
    waits_here: false,
  };
  db.cache_shared_version::<Instruction>(rule, &cached).expect("cached");
}

#[test]
fn a_version_that_met_another_is_taken_back_as_a_draft_on_the_latest_unless_its_base_is_kept() {
  let (_repo, mut db, rule, edit) = team_with_waiting_edit();
  teammate_publishes(&mut db, rule, "Theirs.");
  db.record_publish_conflict(edit, VersionNumber::new(std::num::NonZeroU32::new(2).expect("two")))
    .expect("recorded");

  db.reopen_draft::<Instruction>(edit, at(8)).expect("taken back");
  let draft = db.draft::<Instruction>(rule).expect("read").expect("a draft");
  assert_eq!(
    draft.based_on(),
    Some(VersionNumber::new(std::num::NonZeroU32::new(2).expect("two"))),
    "the plain take-back says the draft is edited from the latest"
  );
}

#[test]
fn a_draft_taken_back_on_its_base_stays_edited_from_the_version_it_was_edited_from() {
  let (_repo, mut db, rule, edit) = team_with_waiting_edit();
  teammate_publishes(&mut db, rule, "Theirs.");
  db.record_publish_conflict(edit, VersionNumber::new(std::num::NonZeroU32::new(2).expect("two")))
    .expect("recorded");

  db.reopen_draft_on_its_base::<Instruction>(edit, at(8))
    .expect("taken back");

  let draft = db.draft::<Instruction>(rule).expect("read").expect("a draft");
  assert_eq!(draft.body(), &instruction("Two."), "what the person wrote is kept");
  assert_eq!(
    draft.based_on(),
    Some(VersionNumber::FIRST),
    "and still says it was edited from v1, so a merge can tell what the team changed since"
  );
  assert!(db.waiting_publishes().expect("outbox").is_empty());
}

#[test]
fn a_draft_taken_back_on_its_base_walks_past_a_base_that_was_not_sent_either() {
  let (_repo, mut db, rule, _first_edit) = team_with_waiting_edit();
  // The person edited again from their own v2, which still waits: v3 is based on a version the server never numbered.
  db.save_draft(
    rule,
    &Draft::new(
      instruction("Three."),
      Some(VersionNumber::new(std::num::NonZeroU32::new(2).expect("two"))),
      at(6),
    ),
  )
  .expect("a draft");
  let second_edit = db
    .publish_draft::<Instruction>(rule, at(7))
    .expect("published here again");
  teammate_publishes(&mut db, rule, "Theirs.");

  db.reopen_draft_on_its_base::<Instruction>(second_edit.id(), at(8))
    .expect("taken back");

  let draft = db.draft::<Instruction>(rule).expect("read").expect("a draft");
  assert_eq!(draft.body(), &instruction("Three."));
  assert_eq!(
    draft.based_on(),
    Some(VersionNumber::FIRST),
    "edited from v1, the last version the server numbered that its chain was edited from, not from the team's newer one"
  );
}

#[test]
fn an_archived_resource_with_a_draft_is_listed_with_when_it_went() {
  let (_repo, mut db, rule, edit) = team_with_waiting_edit();
  db.reopen_draft::<Instruction>(edit, at(6)).expect("taken back");
  assert!(
    db.archived_with_draft(orchester_core::resource::ResourceKind::Instruction)
      .expect("read")
      .is_empty(),
    "a live resource is not archived"
  );

  db.set_resource_sharing(
    rule,
    Some(ResourceSharing::Shared(TaskRevision::try_from(1).expect("revision"))),
  )
  .expect("the team's");
  assert!(db.mark_shared_archived(rule, true, at(9)).expect("archived"));

  let archived = db
    .archived_with_draft(orchester_core::resource::ResourceKind::Instruction)
    .expect("read");
  assert_eq!(archived.len(), 1);
  assert_eq!(archived[0].0.id(), rule);
  assert_eq!(archived[0].1, at(9));
  assert_eq!(draft_text(&db, rule), Some(instruction("Two.")), "the draft is kept");

  assert!(db.mark_shared_archived(rule, false, at(10)).expect("restored"));
  assert!(
    db.archived_with_draft(orchester_core::resource::ResourceKind::Instruction)
      .expect("read")
      .is_empty()
  );
}

#[test]
fn the_archived_mark_and_the_revision_the_server_gave_are_kept_together() {
  let (_repo, mut db, rule, edit) = team_with_waiting_edit();
  db.reopen_draft::<Instruction>(edit, at(6)).expect("taken back");
  db.set_resource_sharing(
    rule,
    Some(ResourceSharing::Shared(TaskRevision::try_from(1).expect("revision"))),
  )
  .expect("the team's");
  assert!(db.mark_shared_archived(rule, true, at(9)).expect("archived"));
  let second = TaskRevision::try_from(2).expect("revision");

  assert!(
    db.mark_shared_archived_at(rule, false, second, at(10))
      .expect("restored")
  );

  assert_eq!(
    db.resource_sharing(rule).expect("read"),
    Some(ResourceSharing::Shared(second))
  );
  assert!(
    db.archived_with_draft(orchester_core::resource::ResourceKind::Instruction)
      .expect("read")
      .is_empty()
  );
}
