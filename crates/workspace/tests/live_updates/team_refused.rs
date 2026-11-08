//! A version this computer knows the team cannot take (a pipeline running a profile the team does not have, a profile
//! with writable folders of this computer) is recorded as refused, as one the server refused is, instead of waiting to
//! be retried: the editors show it, the other versions are still sent, and a profile stuck on it is not the team's.
//! A server that cannot be reached leaves a version waiting.

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::{AgentPermissions, WritableFolders},
  resource::{AgentProfile, Pipeline, Slug},
  sharing::MemberRole,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, PublishState};
use orchester_workspace::sharing::SharingError;

use super::{
  ALICE, TeamServer,
  config::{codex, profile},
  publish::Computer,
  team_copy::{one_step, own_pipeline, own_profile, team_profile},
};

/// A pipeline made here running `profile`, published as a version that waits for the server.
fn waiting_pipeline(db: &mut ProjectDb, slug: &str, profile: ResourceId) -> ResourceId {
  let id = db
    .create_resource(
      Slug::new(slug).expect("slug"),
      Name::new(slug).expect("name"),
      &one_step(profile),
      Timestamp::now(),
    )
    .expect("a pipeline")
    .id();
  db.publish_draft::<Pipeline>(id, Timestamp::now())
    .expect("published here");
  id
}

/// Where the version of `resource` published here stands; `None` once the server numbered it.
fn standing(db: &ProjectDb, resource: ResourceId) -> Option<PublishState> {
  db.waiting_publishes()
    .expect("the outbox")
    .into_iter()
    .find(|waiting| waiting.resource == resource)
    .map(|waiting| waiting.state)
}

fn refused(code: &str) -> Option<PublishState> {
  Some(PublishState::Refused { code: code.to_owned() })
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_pipeline_naming_a_profile_the_team_lacks_is_refused_and_the_others_are_still_sent() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let team = team_profile(&server, &alice).await;
  let own = own_profile(&mut alice.workspace.db());
  let (bad, good) = {
    let mut db = alice.workspace.db();
    let bad = waiting_pipeline(&mut db, "hotfix", own);
    let local = own_pipeline(&mut db, team);
    let good = db
      .copy_for_the_team::<Pipeline>(local, Timestamp::now())
      .expect("copied for the team")
      .id();
    (bad, good)
  };
  assert_eq!(standing(&alice.workspace.db(), bad), Some(PublishState::Waiting));

  alice.send_waiting(&server).await.expect("sent");

  let db = alice.workspace.db();
  assert_eq!(
    standing(&db, bad),
    refused("pipeline_names_non_team"),
    "recorded, not retried"
  );
  assert!(server.versions_of(bad).is_empty(), "nothing of it reached the server");
  assert_eq!(standing(&db, good), None, "the others were still sent");
  assert_eq!(server.versions_of(good).len(), 1);

  alice.send_waiting(&server).await.expect("nothing to send");
  assert_eq!(standing(&alice.workspace.db(), bad), refused("pipeline_names_non_team"));
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_profile_with_writable_folders_is_refused_and_leaves_the_pending_team() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let laptop = codex(&mut alice.workspace.db(), &alice.workspace.root, "Laptop");
  let (stuck, pipeline) = {
    let mut db = alice.workspace.db();
    let folders: WritableFolders = serde_json::from_value(serde_json::json!(["/opt/cache"])).expect("folders");
    let permissions = AgentPermissions::before_permissions().with_writable_folders(folders);
    let body: AgentProfile = profile(laptop, &[], permissions);
    let stuck = db
      .create_resource(
        Slug::new("builder").expect("slug"),
        Name::new("Builder").expect("name"),
        &body,
        Timestamp::now(),
      )
      .expect("a profile")
      .id();
    db.publish_draft::<AgentProfile>(stuck, Timestamp::now())
      .expect("published here");
    (stuck, waiting_pipeline(&mut db, "delivery", stuck))
  };
  assert_eq!(standing(&alice.workspace.db(), stuck), Some(PublishState::Waiting));

  alice.send_waiting(&server).await.expect("sent");

  let db = alice.workspace.db();
  assert_eq!(standing(&db, stuck), refused("writable_folders_stay"));
  assert!(server.versions_of(stuck).is_empty());
  // While the profile waited it was the team's to come, so the pipeline running it could wait too; refused, it is not.
  assert_eq!(standing(&db, pipeline), refused("pipeline_names_non_team"));
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_version_waits_while_the_server_cannot_be_reached() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let team = team_profile(&server, &alice).await;
  let own = own_profile(&mut alice.workspace.db());
  let (bad, good) = {
    let mut db = alice.workspace.db();
    let local = own_pipeline(&mut db, team);
    let good = db
      .copy_for_the_team::<Pipeline>(local, Timestamp::now())
      .expect("copied for the team")
      .id();
    (waiting_pipeline(&mut db, "hotfix", own), good)
  };
  alice.go_offline(true);

  let offline = alice.send_waiting(&server).await;

  assert!(matches!(offline, Err(SharingError::Offline)), "{offline:?}");
  let db = alice.workspace.db();
  assert_eq!(standing(&db, good), Some(PublishState::Waiting));
  assert_eq!(
    standing(&db, bad),
    Some(PublishState::Waiting),
    "not judged until it is sent"
  );

  alice.go_offline(false);
  alice.send_waiting(&server).await.expect("sent");
  let db = alice.workspace.db();
  assert_eq!(standing(&db, good), None);
  assert_eq!(standing(&db, bad), refused("pipeline_names_non_team"));
}
