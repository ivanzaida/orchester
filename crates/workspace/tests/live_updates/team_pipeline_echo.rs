//! A team pipeline's save memory took once the server took it (PV2-34, D-043) comes back on the project's stream as
//! its `resource.published`: the stream finds the version memory already holds and keeps it once, without an error,
//! and the stream stays live. Nothing of it is written to the project's database.

use orchester_api::SharedResourceKind;
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    CommandAction, CommandProgram, Pipeline, PipelineEdit, ResourceKind, Slug, StepKind, Tool, ToolAction,
    VersionNumber,
  },
  sharing::MemberRole,
  store::ResourceStore,
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

/// The tool *Cargo* the team server has, which the stream's read at its open brings to memory.
fn team_tool(server: &TeamServer) -> ResourceId {
  let id = ResourceId::generate();
  let program = CommandProgram::new("cargo").expect("a program");
  let tool = Tool::new("", ToolAction::Command(CommandAction::new(program, vec![])), None).expect("a tool");
  let body = serde_json::to_value(&tool).expect("a tool serializes");
  server.publish((id, SharedResourceKind::Tool, "Cargo"), body, Vec::new());
  id
}

fn number(value: u32) -> VersionNumber {
  VersionNumber::new(std::num::NonZeroU32::new(value).expect("a version number"))
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_team_pipelines_save_coming_back_on_the_stream_is_kept_once() {
  // Alice, the first member, is the fake server's admin.
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &["Status bar"]);
  let tool = team_tool(&server);
  let desktop = Desktop::signed_in_as(&server, (ALICE, MemberRole::Owner), Fixture::new(), &[]);
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
  let held = |resource| {
    let read = desktop.workspace.team_read(desktop.root());
    read.config().and_then(|config| config.resource(resource).cloned())
  };
  seen_within(SETTLED, "the team's tool in memory", || held(tool).is_some()).await;

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

  // Both `resource.published` reach this computer: the stream's cursor is the server's.
  let db = || ProjectDb::open(desktop.root()).expect("the project database");
  seen_within(SETTLED, "the stream's echo", || {
    db()
      .sharing()
      .expect("read")
      .is_some_and(|link| link.cursor == server.cursor())
  })
  .await;
  let pipeline = held(id).expect("memory holds the pipeline");
  let body = pipeline.body::<Pipeline>().expect("a version").expect("a pipeline");
  assert_eq!(
    (pipeline.number(), body.description()),
    (Some(number(2)), "Changed."),
    "the latest version, as the save left it"
  );
  let read = desktop.workspace.team_read(desktop.root());
  let pipelines = read
    .config()
    .map(|config| config.resources(ResourceKind::Pipeline).len());
  assert_eq!(pipelines, Some(1), "the pipeline once");
  assert!(
    db().resources(ResourceKind::Pipeline).expect("read").is_empty(),
    "nothing of the team's is written to the project's database"
  );
  assert_eq!(server.versions_of(id).len(), 2);
  assert_eq!(
    state(),
    Some(SyncState::Live),
    "the stream took its echo without an error"
  );
  streams.abort();
}
