//! A stream resumed from a cursor that came without the snapshot, as Join leaves it: the team's configuration published
//! before still reaches the cache.

use orchester_api::SharedResourceKind;
use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, Pipeline, VersionNumber},
  time::Timestamp,
};
use orchester_store::SharedEvent;

use super::{
  ALICE, BOB, Member, SETTLED, TeamServer, Workspace,
  config::{agent, codex, latest, pipeline, step, team_builder, team_references},
  seen_within,
};

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_stream_resumed_without_the_snapshot_keeps_the_configuration_published_before() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")], &["Status bar"]);
  let (profile, delivery) = (ResourceId::generate(), ResourceId::generate());
  server.publish(
    (profile, SharedResourceKind::AgentProfile, "Builder"),
    team_builder(&[]),
    team_references(&[]),
  );
  server.publish(
    (delivery, SharedResourceKind::Pipeline, "Delivery"),
    pipeline(vec![step("implement", agent(profile))]),
    Vec::new(),
  );
  let workspace = Workspace::linked(&server);
  codex(&mut workspace.db(), &workspace.root, "Laptop");
  let joined = server.cursor();
  workspace
    .db()
    .take_shared_event(&SharedEvent::default(), joined, Timestamp::now())
    .expect("the cursor Join keeps");

  let stream = workspace.follow(Member::of(&server, BOB));
  seen_within(SETTLED, "the resources published before", || {
    latest::<Pipeline>(&workspace, delivery).is_some()
  })
  .await;
  assert!(
    workspace
      .db()
      .cached_version::<AgentProfile>(profile, VersionNumber::FIRST)
      .expect("read")
      .is_some(),
    "the profile its step runs"
  );
  assert_eq!(server.connects(), [Some(joined)], "resumed, no snapshot");
  stream.abort();
}
