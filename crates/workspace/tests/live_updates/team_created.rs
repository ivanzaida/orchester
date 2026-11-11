//! A new resource created here as its first version, which the team server refuses when it is sent later (after a
//! restart, or the server back), is taken back whole, but only when nothing else of the person's goes with it: a later
//! version or a draft on it, or a version the server numbered, keeps the resource, refused, for the admin to decide.
//! One resource that cannot be taken back does not stop the others from being sent, and a first version published from
//! a draft is never taken back.

use std::collections::BTreeSet;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceKind, Slug, VersionNumber,
  },
  sharing::MemberRole,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, PublishState};
use orchester_workspace::sharing::TakenBack;

use super::{
  ALICE, BOB, TeamServer,
  publish::Computer,
};

fn rule(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("instruction")
}

/// A team where only Alice changes the configuration, and Bob's computer, which thinks he may.
fn team() -> (TeamServer, Computer, Computer) {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Weber")], &[]);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let bob = Computer::new(&server, BOB, MemberRole::Member);
  bob
    .workspace
    .db()
    .set_server_admin(true)
    .expect("believes he is an admin");
  (server, alice, bob)
}

/// A new instruction created here as v1, the way the hub's form does.
fn created(db: &mut ProjectDb, slug: &str) -> ResourceId {
  let slug_text = Slug::new(slug).expect("slug");
  db.create_published(slug_text, Name::new(slug).expect("name"), &rule(slug), Timestamp::now())
    .expect("created here")
    .0
    .id()
}

/// Version 2 of `resource`, published over its v1.
fn publish_v2(db: &mut ProjectDb, resource: ResourceId) {
  save_draft(db, resource);
  db.publish_draft::<Instruction>(resource, Timestamp::now()).expect("v2");
}

fn save_draft(db: &mut ProjectDb, resource: ResourceId) {
  db.save_draft(
    resource,
    &Draft::new(rule("Edited."), Some(VersionNumber::FIRST), Timestamp::now()),
  )
  .expect("a draft over v1");
}

fn standings(db: &ProjectDb, resource: ResourceId) -> Vec<(u32, PublishState)> {
  db.waiting_publishes()
    .expect("the outbox")
    .into_iter()
    .filter(|waiting| waiting.resource == resource)
    .map(|waiting| (waiting.number.get(), waiting.state))
    .collect()
}

/// The instructions kept in the database, those a team project does not offer while the server refused them too.
fn held(db: &ProjectDb) -> usize {
  db.kept_resources(ResourceKind::Instruction)
    .expect("the instructions")
    .len()
}

fn not_admin() -> PublishState {
  PublishState::Refused {
    code: "not_admin".to_owned(),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_lone_created_v1_the_server_refuses_later_is_taken_back_and_told() {
  let (server, _alice, bob) = team();
  let rule = created(&mut bob.workspace.db(), "release-notes");

  let taken = bob.send_waiting(&server).await.expect("sent");

  assert_eq!(taken.len(), 1, "{taken:?}");
  let TakenBack {
    resource, name, kind, ..
  } = &taken[0];
  assert_eq!(
    (*resource, name.as_str(), *kind),
    (rule, "release-notes", ResourceKind::Instruction)
  );
  let db = bob.workspace.db();
  assert_eq!(held(&db), 0, "taken back whole");
  assert!(db.waiting_publishes().expect("the outbox").is_empty());
  assert!(server.versions_of(rule).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_refused_v1_with_a_later_version_or_a_draft_on_it_keeps_both_and_stays_refused() {
  let (server, _alice, bob) = team();
  let (later, drafted) = {
    let mut db = bob.workspace.db();
    let later = created(&mut db, "later");
    publish_v2(&mut db, later);
    let drafted = created(&mut db, "drafted");
    save_draft(&mut db, drafted);
    (later, drafted)
  };

  let taken = bob.send_waiting(&server).await.expect("sent");

  assert!(taken.is_empty(), "nothing was taken back: {taken:?}");
  let db = bob.workspace.db();
  assert_eq!(held(&db), 2);
  assert_eq!(
    db.versions::<Instruction>(later).expect("versions").len(),
    2,
    "v2 is kept"
  );
  assert_eq!(
    standings(&db, later)[0],
    (1, not_admin()),
    "v2 waits behind the refused v1"
  );
  assert!(
    db.draft::<Instruction>(drafted).expect("a draft read").is_some(),
    "the edits are kept"
  );
  assert_eq!(standings(&db, drafted), [(1, not_admin())]);
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_v1_the_server_numbered_keeps_its_resource_when_v2_is_refused_and_the_others_are_still_sent() {
  let (server, alice, _bob) = team();
  let (numbered, other) = {
    let mut db = alice.workspace.db();
    let numbered = created(&mut db, "numbered");
    publish_v2(&mut db, numbered);
    (numbered, created(&mut db, "other"))
  };
  server.refuse_versions(true);

  let taken = alice.send_waiting(&server).await.expect("the pass went on");

  assert!(taken.is_empty(), "{taken:?}");
  let db = alice.workspace.db();
  assert_eq!(held(&db), 2, "the team's resource is kept");
  assert_eq!(
    standings(&db, numbered),
    [(2, not_admin())],
    "v1 was numbered; v2 waits for the admin"
  );
  assert_eq!(server.versions_of(numbered).len(), 1);
  assert!(standings(&db, other).is_empty(), "the resource after it was still sent");
  assert_eq!(server.versions_of(other).len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_first_version_published_from_a_draft_is_not_taken_back_when_refused() {
  let (server, _alice, bob) = team();
  // A pipeline is never a draft (it is created as its v1); an instruction copied here as a draft still is.
  let imported = {
    let mut db = bob.workspace.db();
    let imported = db
      .create_resource(
        Slug::new("imported").expect("slug"),
        Name::new("Imported").expect("name"),
        &rule("Imported."),
        Timestamp::now(),
      )
      .expect("a draft")
      .id();
    db.publish_draft::<Instruction>(imported, Timestamp::now())
      .expect("v1 from the draft");
    imported
  };

  let taken = bob.send_waiting(&server).await.expect("sent");

  assert!(taken.is_empty(), "{taken:?}");
  let db = bob.workspace.db();
  assert!(
    db.resource(imported).expect("read").is_some(),
    "the work of a draft is never discarded"
  );
  assert_eq!(standings(&db, imported), [(1, not_admin())]);
}
