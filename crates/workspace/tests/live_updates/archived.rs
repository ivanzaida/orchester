//! A team resource archived on the server leaves this computer's lists and stops the runs that pin it
//! (`Source::archived`); restored, it is back. The snapshot of a project whose resource is archived already keeps
//! it out without stopping anything twice.

use std::sync::Arc;

use orchester_api::SharedResourceKind;
use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, ResourceKind, VersionNumber},
  store::ResourceStore,
};
use orchester_workspace::sharing::sync::{SystemClock, follow};

use super::{
  ALICE, LIVE, Member, TeamServer, Workspace,
  config::{team_builder, team_references},
  seen_within,
};

fn listed(workspace: &Workspace, resource: ResourceId) -> bool {
  let profiles = workspace.db().resources(ResourceKind::AgentProfile).expect("listed");
  profiles.iter().any(|kept| kept.id() == resource)
}

fn archived(member: &Member) -> Vec<ResourceId> {
  member.archived.lock().expect("archived").clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_an_archived_profile_leaves_the_list_and_stops_its_runs_until_restored() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let profile = ResourceId::generate();
  server.publish(
    (profile, SharedResourceKind::AgentProfile, "Builder"),
    team_builder(&[]),
    team_references(&[]),
  );
  let workspace = Workspace::linked(&server);
  let member = Arc::new(Member::of(&server, ALICE));
  let following = {
    let (member, root) = (member.clone(), workspace.root.clone());
    tokio::spawn(async move { follow(&*member, &root, &SystemClock).await })
  };
  seen_within(LIVE, "the snapshot's profile", || listed(&workspace, profile)).await;

  server.archive(profile, true);
  seen_within(LIVE, "the profile leaves the list", || !listed(&workspace, profile)).await;
  assert_eq!(archived(&member), [profile], "its runs are stopped once");
  assert!(
    workspace.db().resource_removed_at(profile).expect("read").is_some(),
    "removed here, so no new run pins it"
  );
  assert!(
    workspace
      .db()
      .cached_version::<AgentProfile>(profile, VersionNumber::FIRST)
      .expect("read")
      .is_some(),
    "its versions stay for the runs that pinned them"
  );

  server.archive(profile, false);
  seen_within(LIVE, "the profile is back", || listed(&workspace, profile)).await;
  assert_eq!(archived(&member), [profile], "a restore stops nothing");
  following.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_snapshot_keeps_an_archived_profile_out_of_the_list() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (profile, other) = (ResourceId::generate(), ResourceId::generate());
  for (id, name) in [(profile, "Builder"), (other, "Reviewer")] {
    server.publish(
      (id, SharedResourceKind::AgentProfile, name),
      team_builder(&[]),
      team_references(&[]),
    );
  }
  server.archive(profile, true);
  let workspace = Workspace::linked(&server);
  let member = Arc::new(Member::of(&server, ALICE));
  let following = {
    let (member, root) = (member.clone(), workspace.root.clone());
    tokio::spawn(async move { follow(&*member, &root, &SystemClock).await })
  };
  seen_within(LIVE, "the snapshot's other profile", || listed(&workspace, other)).await;
  assert!(!listed(&workspace, profile), "archived before this computer joined");
  assert_eq!(archived(&member), [profile], "kept as archived, its runs are checked");
  following.abort();
}
