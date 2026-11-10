//! A team resource archived on the server leaves the team's configuration this computer reads (in memory) and stops
//! the runs that pin it (`Source::archived`); restored, it is back. A project whose resource is archived already reads
//! it archived when its stream opens, and its runs are checked then.

use std::sync::Arc;

use orchester_api::SharedResourceKind;
use orchester_core::{id::ResourceId, resource::ResourceKind, store::ResourceStore};
use orchester_workspace::sharing::sync::{SystemClock, follow};

use super::{
  ALICE, Member, SETTLED, TeamServer, Workspace,
  config::{team_builder, team_references},
  seen_within,
};

/// Whether the team's configuration `member`'s stream read has `resource`, not archived.
fn listed((member, workspace): (&Member, &Workspace), resource: ResourceId) -> bool {
  member
    .configs
    .read(&workspace.root)
    .config()
    .is_some_and(|config| config.live(resource).is_some())
}

/// Nothing of the team's is written to the project's database.
fn nothing_kept(workspace: &Workspace) {
  let profiles = workspace.db().resources(ResourceKind::AgentProfile).expect("read");
  assert!(
    profiles.is_empty(),
    "no team profile in the project's database: {profiles:?}"
  );
}

fn archived(member: &Member) -> Vec<(ResourceId, Option<String>)> {
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
  seen_within(SETTLED, "the snapshot's profile", || {
    listed((&member, &workspace), profile)
  })
  .await;

  server.archive(profile, true);
  seen_within(SETTLED, "the profile leaves the list", || {
    !listed((&member, &workspace), profile)
  })
  .await;
  assert_eq!(
    archived(&member),
    [(profile, Some("Alice Moreau".to_owned()))],
    "its runs are stopped once, and the stream says who archived it"
  );
  nothing_kept(&workspace);

  server.archive(profile, false);
  seen_within(SETTLED, "the profile is back", || {
    listed((&member, &workspace), profile)
  })
  .await;
  assert_eq!(
    archived(&member),
    [(profile, Some("Alice Moreau".to_owned()))],
    "a restore stops nothing"
  );
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
  seen_within(SETTLED, "the snapshot's other profile", || {
    listed((&member, &workspace), other)
  })
  .await;
  assert!(
    !listed((&member, &workspace), profile),
    "archived before this computer joined"
  );
  assert_eq!(
    archived(&member),
    [(profile, None)],
    "read archived, its runs are checked, and the read does not say who archived it"
  );
  nothing_kept(&workspace);
  following.abort();
}
