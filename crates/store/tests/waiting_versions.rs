//! A teammate's version that waits for *Use v<N> here* (ORC-387): every read of the version used here leaves it out,
//! so neither Always allow nor an editor builds on what it adds, and a version published here supersedes it.
//! `cargo test -p orchester-store --test waiting_versions waiting_versions`.

mod support;

use std::num::NonZeroU32;

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ResourceId, UserId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandScope},
  },
  resource::{AgentProfile, Draft, ModelId, PromptText, ProviderSettings, PublishError, Slug, VersionNumber},
  sharing::SharedPerson,
  store::ResourceStore,
};
use orchester_store::{CachedVersion, ProjectDb, ResourceUpdate, StoreError};
use serde_json::json;

use self::support::{at, name, started_run};

fn number(value: u32) -> VersionNumber {
  VersionNumber::new(NonZeroU32::new(value).expect("above zero"))
}

fn profile(prefixes: &[&str]) -> AgentProfile {
  let entries = prefixes
    .iter()
    .map(|prefix| CommandPrefix::new(prefix).expect("prefix"))
    .collect();
  AgentProfile::new(
    name("Designer"),
    String::new(),
    PromptText::new("Design.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("claude-opus-5-5").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(AllowedCommands::new(entries).expect("list")),
    NetworkAccess::On,
  ))
}

/// A profile published here as v1, and a teammate's v2 that allows `curl` too and waits.
fn waiting_v2(db: &mut ProjectDb) -> (ResourceId, AgentProfile) {
  let v1 = profile(&["cargo test"]);
  let id = db
    .create_resource(Slug::new("designer").expect("slug"), name("Designer"), &v1, at(1))
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, at(2)).expect("v1");
  let v1 = db
    .latest_version::<AgentProfile>(id)
    .expect("read")
    .expect("v1")
    .body()
    .clone();
  cache_waiting(db, id, 2);
  assert_eq!(db.waiting_version(id).expect("waiting"), Some(number(2)));
  (id, v1)
}

fn cache_waiting(db: &mut ProjectDb, id: ResourceId, value: u32) {
  let version = CachedVersion {
    number: number(value),
    published_at: at(3),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Olena".to_owned(),
    },
    server_body: json!({ "schema": 1, "body": {} }),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references: Vec::new(),
    local_body: profile(&["cargo test", "curl"]),
    waits_here: true,
  };
  db.cache_shared_version(id, &version).expect("cached");
}

fn latest(db: &ProjectDb, id: ResourceId) -> (u32, AgentProfile) {
  let version = db.latest_version::<AgentProfile>(id).expect("read").expect("a version");
  (version.number().get(), version.body().clone())
}

#[test]
fn waiting_versions_always_allow_builds_on_the_version_used_here() {
  let (repo, mut db, ..) = started_run();
  let (id, v1) = waiting_v2(&mut db);
  let scope = CommandScope::new(repo.path(), &[], None, None);
  let command = ClassifiedCommand::classify("cargo add serde", &scope);
  let prefix = CommandPrefix::new("cargo add").expect("prefix");

  let published = db
    .publish_allowed_command(id, &command, &prefix, at(4))
    .expect("published");

  assert_eq!(published, number(3), "numbered after the waiting v2");
  let (used, body) = latest(&db, id);
  assert_eq!(used, 3);
  assert_eq!(
    body.permissions(),
    profile(&["cargo test", "cargo add"]).permissions(),
    "v2's curl never reaches runs without Use v2 here"
  );
  assert_eq!(
    body.with_permissions(v1.permissions().clone()),
    v1,
    "otherwise v3 is v1"
  );
  assert_eq!(db.waiting_version(id).expect("waiting"), None, "v3 supersedes v2");
}

#[test]
fn waiting_versions_editors_save_and_publish_over_the_version_used_here() {
  let (_repo, mut db, ..) = started_run();
  let (id, v1) = waiting_v2(&mut db);
  let edited = profile(&["cargo test", "cargo check"]);

  db.update_draft::<AgentProfile>(id, |draft, latest| {
    assert!(draft.is_none());
    let latest = latest.expect("v1");
    assert_eq!((latest.number(), latest.body()), (number(1), &v1));
    Ok(Draft::new(edited.clone(), Some(latest.number()), at(4)))
  })
  .expect("update");
  let again = profile(&["cargo test", "cargo check", "cargo fmt"]);
  db.save_draft_matching(
    id,
    &edited,
    Some(number(1)),
    &Draft::new(again.clone(), Some(number(1)), at(5)),
  )
  .expect("an editor that read v1 saves while v2 waits");
  db.update_resource::<AgentProfile, StoreError>(id, |_, draft, latest| {
    assert_eq!(draft.expect("draft").body(), &again);
    assert_eq!(latest.expect("v1").number(), number(1));
    Ok(ResourceUpdate::SaveDraft(Draft::new(
      again.clone(),
      Some(number(1)),
      at(6),
    )))
  })
  .expect("resource update");

  let version = db
    .publish_draft_matching_version(id, &again, Some(number(1)), at(7))
    .expect("published over v1");
  assert_eq!(version.number(), number(3));
  assert_eq!(latest(&db, id), (3, again));
  assert_eq!(db.waiting_version(id).expect("waiting"), None);
}

#[test]
fn waiting_versions_a_draft_based_on_a_waiting_version_does_not_publish() {
  let (_repo, mut db, ..) = started_run();
  let (id, _) = waiting_v2(&mut db);
  db.save_draft(id, &Draft::new(profile(&["curl"]), Some(number(2)), at(4)))
    .expect("draft");

  let refused = db.publish_draft::<AgentProfile>(id, at(5));
  assert!(matches!(
    refused,
    Err(StoreError::Publish(PublishError::StaleDraft { based_on, latest }))
      if based_on == Some(number(2)) && latest == Some(number(1))
  ));
  let refused = db.publish_draft_with::<AgentProfile>(id, at(5), |draft, latest| {
    assert_eq!(latest.expect("v1").number(), number(1));
    Ok(draft)
  });
  assert!(matches!(
    refused,
    Err(StoreError::Publish(PublishError::StaleDraft { .. }))
  ));
  assert_eq!(latest(&db, id).0, 1);
  assert_eq!(db.waiting_version(id).expect("waiting"), Some(number(2)));
}
