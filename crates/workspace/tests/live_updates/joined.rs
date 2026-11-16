//! A stream resumed from a cursor that came without the snapshot, as Join leaves it: the team's configuration published
//! before is read into memory when the stream opens, and none of it is written to the project's database.

use std::sync::Arc;

use orchester_api::SharedResourceKind;
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::SharedEvent;
use orchester_workspace::sharing::sync::{SystemClock, follow};

use super::{
  ALICE, BOB, Member, SETTLED, TeamServer, Workspace,
  config::{agent, codex, pipeline, step, team_builder, team_references},
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

  let member = Arc::new(Member::of(&server, BOB));
  let stream = {
    let (member, root) = (member.clone(), workspace.root.clone());
    tokio::spawn(async move { follow(&*member, &root, &SystemClock).await })
  };
  let read = || member.configs.read(&workspace.root).config().cloned();
  seen_within(SETTLED, "the resources published before", || {
    read().is_some_and(|config| config.resource(delivery).is_some())
  })
  .await;
  let config = read().expect("read");
  let pipeline = config.resource(delivery).expect("Delivery");
  assert!(
    matches!(pipeline.body::<Pipeline>(), Some(Ok(_))),
    "its latest version is read"
  );
  assert!(config.resource(profile).is_some(), "the profile its step runs");
  assert!(
    workspace
      .db()
      .resources(ResourceKind::Pipeline)
      .expect("read")
      .is_empty(),
    "nothing of the team's is written to the project's database"
  );
  assert_eq!(server.connects(), [Some(joined)], "resumed, no snapshot");
  stream.abort();
}
