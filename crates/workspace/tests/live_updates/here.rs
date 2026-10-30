//! Sharing › On this computer as a resource's page reads it (`Here::missing_for`, `Here::connection_of`): only what the
//! version runs use here names counts, not what an older cached version named.

use orchester_api::{ReferenceTarget, SharedResourceKind};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  resource::AgentProfile,
};
use orchester_store::BoundBy;
use orchester_workspace::sharing::sync::{Here, Standing};

use super::{
  ALICE, LIVE, Member, TeamServer, Workspace,
  config::{builder, codex, latest, reference, use_first},
  seen_within,
};

/// Builder's v1 named the API connection `DeepSeek`, which this computer has nothing for; v2, which runs use here,
/// names Codex `Work`, bound by its name. Only what v2 names counts for the profile's page.
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_on_this_computer_counts_only_the_version_runs_use_for_a_resource() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (deepseek, theirs, profile) = (
    ConnectionId::generate(),
    ConnectionId::generate(),
    ResourceId::generate(),
  );
  server.publish(
    (profile, SharedResourceKind::AgentProfile, "Builder"),
    builder(deepseek, &[]),
    vec![reference(
      ReferenceTarget::Connection { id: deepseek },
      "DeepSeek",
      Some("openai_api"),
    )],
  );
  let workspace = Workspace::linked(&server);
  let work = codex(&mut workspace.db(), &workspace.root, "work");
  let stream = workspace.follow(Member::of(&server, ALICE));
  use_first(&workspace, profile).await;
  let here = Here::read(&workspace.root).expect("read");
  assert_eq!(here.missing_for(profile).len(), 1, "v1 names DeepSeek, missing here");

  server.publish(
    (profile, SharedResourceKind::AgentProfile, "Builder"),
    builder(theirs, &[]),
    vec![reference(
      ReferenceTarget::Connection { id: theirs },
      "Work",
      Some("codex_cli"),
    )],
  );
  seen_within(LIVE, "v2 from its event", || {
    latest::<AgentProfile>(&workspace, profile).is_some_and(|version| version.number().get() == 2)
  })
  .await;
  let here = Here::read(&workspace.root).expect("read again");
  assert_eq!(
    here.needs_you(),
    1,
    "DeepSeek still has its row on Sharing › On this computer"
  );
  assert!(
    here.missing_for(profile).is_empty(),
    "what only v1 names doesn't keep Builder from running here"
  );
  let runs_here = here.connection_of(profile).expect("v2 names a connection");
  assert_eq!(runs_here.binding.name, "Work");
  assert_eq!(
    runs_here.standing,
    Standing::Bound {
      local: "work".into(),
      by: BoundBy::Name
    }
  );
  assert_eq!(
    latest::<AgentProfile>(&workspace, profile)
      .expect("v2")
      .body()
      .connection(),
    work
  );
  stream.abort();
}
