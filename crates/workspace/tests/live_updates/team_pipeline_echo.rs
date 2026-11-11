//! A team pipeline's save kept here once the server took it (PV2-34, D-043) comes back on the project's stream as its
//! `resource.published`: the stream finds the version already kept and keeps it once, without an error, and the
//! stream stays live.

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Pipeline, PipelineEdit, Resource, ResourceKind, Slug, StepKind, VersionNumber},
  sharing::{MemberRole, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_ipc::SessionEnd;
use orchester_store::ProjectDb;
use orchester_workspace::{activity::Activity, runner::Runners, sharing::sync::SyncState};

use super::{ALICE, Desktop, SETTLED, TeamServer, seen_within, support::Fixture};

/// A pipeline of one step running `tool`, described `text`.
fn checks(tool: ResourceId, text: &str) -> Pipeline {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("an empty body"));
  let key = edit.add_step(Name::new("Check").expect("name"), None);
  edit.update(&key, |step| step.kind = StepKind::Tool { tool: Some(tool) });
  edit.set_description(text);
  edit.validate().expect("a valid pipeline")
}

/// A tool of the team's, as the stream keeps it.
fn team_tool(db: &mut ProjectDb) -> ResourceId {
  let id = ResourceId::generate();
  let resource = Resource::new(
    id,
    ResourceKind::Tool,
    Slug::new("cargo").expect("slug"),
    Name::new("Cargo").expect("name"),
    Timestamp::now(),
  );
  db.keep_shared_resource(&resource, TaskRevision::FIRST)
    .expect("the team's tool");
  id
}

fn number(value: u32) -> VersionNumber {
  VersionNumber::new(std::num::NonZeroU32::new(value).expect("a version number"))
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_team_pipelines_save_coming_back_on_the_stream_is_kept_once() {
  // Alice, the first member, is the fake server's admin.
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &["Status bar"]);
  let mut fixture = Fixture::new();
  let tool = team_tool(&mut fixture.db);
  let desktop = Desktop::signed_in_as(&server, (ALICE, MemberRole::Owner), fixture, &[]);
  let streams = tokio::spawn(orchester_workspace::sharing::sync::run(
    desktop.servers.clone(),
    Runners::new(
      "no-runner".into(),
      tokio::runtime::Handle::current(),
      Activity::new(),
      SessionEnd::Clean,
    ),
  ));
  let state = || desktop.servers.sync_state(desktop.root());
  seen_within(SETTLED, "live", || state() == Some(SyncState::Live)).await;

  let (servers, root) = (desktop.servers.clone(), desktop.root().to_owned());
  let saved = tokio::task::spawn_blocking(move || {
    let id = servers.create_team_pipeline(
      &root,
      (Slug::new("checks").expect("slug"), Name::new("Checks").expect("name")),
      &checks(tool, "Ours."),
    )?;
    let second = servers.publish_team_pipeline(&root, id, number(1), &checks(tool, "Changed."))?;
    Ok::<_, orchester_workspace::sharing::NotTaken>((id, second))
  })
  .await
  .expect("the work finished");
  let (id, second) = saved.expect("the server took both");
  assert_eq!(second, number(2));

  // Both `resource.published` reach the cache: the stream's cursor is the server's.
  let db = || ProjectDb::open(desktop.root()).expect("the project database");
  seen_within(SETTLED, "the stream's echo", || {
    db()
      .sharing()
      .expect("read")
      .is_some_and(|link| link.cursor == server.cursor())
  })
  .await;
  let versions = db().versions::<Pipeline>(id).expect("read");
  let kept: Vec<(u32, String)> = versions
    .iter()
    .map(|version| (version.number().get(), version.body().description().to_owned()))
    .collect();
  assert_eq!(
    kept,
    [(1, "Ours.".to_owned()), (2, "Changed.".to_owned())],
    "each version once"
  );
  assert_eq!(server.versions_of(id).len(), 2);
  assert!(
    db().waiting_publishes().expect("the outbox").is_empty(),
    "nothing waits"
  );
  assert_eq!(
    state(),
    Some(SyncState::Live),
    "the stream took its echo without an error"
  );
  streams.abort();
}
