//! A teammate's version that widens what agents may do here (`docs/architecture/server.md`, row 35, "A teammate's
//! version that widens"): it is cached, but runs keep the version before it until the person's *Use v<N> here*, and
//! the resource says what it adds. No MCP tool and no automation presses *Use v<N> here*. A version that does not
//! widen, and one this computer published itself, apply at once.

use std::num::NonZeroU32;

use orchester_api::{ReferenceTarget, SharedResourceKind};
use orchester_core::{
  connection::mcp::{McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, widening::Widening},
  resource::{AgentProfile, CommandProgram, Draft, Pipeline, RepositoryAccess, Slug, TeamMcpServer, VersionNumber},
  run::{Answerer, PreflightProblem},
  sharing::{MemberRole, VersionWidening},
  store::{McpServerStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{PublishState, ResourceSharing};
use orchester_workspace::{
  app_mcp,
  orchestrator::tools,
  sharing::{
    SharingError,
    sync::{Waiting, use_here, waiting},
  },
};

use super::{
  ALICE, LIVE, Member, TeamServer, Workspace,
  config::{
    agent, codex, key, latest, pipeline, problems, profile, step, team_builder, team_profile, team_references,
    use_first,
  },
  publish::Computer,
  seen_within,
};

fn number(value: u32) -> VersionNumber {
  VersionNumber::new(NonZeroU32::new(value).expect("above zero"))
}

fn used(workspace: &Workspace, resource: ResourceId) -> Option<u32> {
  latest::<AgentProfile>(workspace, resource).map(|version| version.number().get())
}

/// Builder, which runs with Codex, and pipeline Delivery that runs it.
struct Team {
  server: TeamServer,
  builder: ResourceId,
  delivery: ResourceId,
}

impl Team {
  fn publish() -> Self {
    let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
    let (builder_id, delivery) = (ResourceId::generate(), ResourceId::generate());
    server.publish(
      (builder_id, SharedResourceKind::AgentProfile, "Builder"),
      team_builder(&[]),
      team_references(&[]),
    );
    server.publish(
      (delivery, SharedResourceKind::Pipeline, "Delivery"),
      pipeline(vec![step("implement", agent(builder_id))]),
      Vec::new(),
    );
    Self {
      server,
      builder: builder_id,
      delivery,
    }
  }

  /// A workspace with a Codex connection, which follows the server until Builder's v1, which runs commands, waits;
  /// with the stream.
  async fn followed(&self) -> (Workspace, tokio::task::JoinHandle<super::Stopped>) {
    let workspace = Workspace::linked(&self.server);
    let mut db = workspace.db();
    codex(&mut db, &workspace.root, "Laptop");
    drop(db);
    let stream = workspace.follow(Member::of(&self.server, ALICE));
    seen_within(LIVE, "the snapshot's resources", || {
      let builder_waits = workspace.db().waiting_version(self.builder).ok().flatten().is_some();
      latest::<Pipeline>(&workspace, self.delivery).is_some() && builder_waits
    })
    .await;
    (workspace, stream)
  }
}

/// *Use v2 here* is not automation's: it is refused and changes nothing.
fn refused_to_automation(workspace: &Workspace, builder_id: ResourceId) {
  assert!(matches!(
    use_here(&workspace.root, builder_id, number(2), Answerer::Automation),
    Err(SharingError::PersonOnly)
  ));
  assert_eq!(used(workspace, builder_id), Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_widening_version_waits_and_runs_keep_the_previous_one_until_use_here() {
  let team = Team::publish();
  let (builder_id, delivery) = (team.builder, team.delivery);
  let (workspace, stream) = team.followed().await;
  assert_eq!(used(&workspace, builder_id), None, "no version is used here yet");
  assert_eq!(
    waiting(&workspace.root, builder_id).expect("read"),
    Some(Waiting {
      number: VersionNumber::FIRST,
      used: None,
      adds: vec![VersionWidening::Permissions(Widening::Commands)],
    }),
    "a first version that runs commands waits too"
  );
  use_first(&workspace, builder_id).await;
  assert_eq!(used(&workspace, builder_id), Some(1));

  let their_docs = ResourceId::generate();
  let wider = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  let v2 = team_profile(&[their_docs], wider);
  team.server.publish(
    (builder_id, SharedResourceKind::AgentProfile, "Builder"),
    serde_json::to_value(&v2).expect("a profile serializes"),
    ReferenceTarget::of_profile(&v2),
  );
  seen_within(LIVE, "v2 from its event", || {
    workspace.db().waiting_version(builder_id).ok().flatten().is_some()
  })
  .await;
  assert_eq!(used(&workspace, builder_id), Some(1), "runs keep v1");
  assert_eq!(
    problems(&workspace, delivery),
    [],
    "a run here takes v1, which needs no MCP server and runs on the only Codex connection"
  );
  assert_eq!(
    waiting(&workspace.root, builder_id).expect("read"),
    Some(Waiting {
      number: number(2),
      used: Some(number(1)),
      adds: vec![
        VersionWidening::Permissions(Widening::Repository),
        VersionWidening::Permissions(Widening::Network),
        VersionWidening::McpServer,
      ],
    }),
    "the resource says what v2 adds"
  );
  refused_to_automation(&workspace, builder_id);

  assert!(
    use_here(&workspace.root, builder_id, number(2), Answerer::Person).expect("used"),
    "the person's Use v2 here"
  );
  assert_eq!(used(&workspace, builder_id), Some(2), "runs take v2 now");
  assert_eq!(waiting(&workspace.root, builder_id).expect("read"), None);
  assert_eq!(
    problems(&workspace, delivery),
    [PreflightProblem::McpServerMissing { step: key("implement") }],
    "v2's MCP server is not set up here"
  );
  stream.abort();
}

/// Both rules at once (rows 35 and 36): the owner's own version, published offline, waits for the server's number and
/// never for *Use v<N> here*; a teammate's widening version that took its number meanwhile waits for the person.
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_teammates_widening_version_waits_while_the_owners_own_version_waits_for_its_number() {
  let team = Team::publish();
  let builder_id = team.builder;
  let alice = Computer::new(&team.server, ALICE, MemberRole::Owner);
  let workspace = &alice.workspace;
  codex(&mut workspace.db(), &workspace.root, "Laptop");
  let stream = workspace.follow(Member::of(&team.server, ALICE));
  use_first(workspace, builder_id).await;
  stream.abort();
  let v1 = latest::<AgentProfile>(workspace, builder_id).expect("v1 used here");

  alice.go_offline(true);
  let narrower = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  );
  let own = v1.body().clone().with_permissions(narrower);
  let draft = Draft::new(own.clone(), Some(number(1)), Timestamp::now());
  let published = alice
    .store(&team.server, move |store| {
      store.save_draft(builder_id, &draft)?;
      store.publish_draft::<AgentProfile>(builder_id, Timestamp::now())
    })
    .await
    .expect("published here");
  assert_eq!(published.number(), number(2), "its number here until the server's");
  assert_eq!(
    used(workspace, builder_id),
    Some(2),
    "the owner's new runs use it at once"
  );

  let wider = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  team.server.publish(
    (builder_id, SharedResourceKind::AgentProfile, "Builder"),
    serde_json::to_value(team_profile(&[], wider)).expect("a profile serializes"),
    team_references(&[]),
  );
  alice.go_offline(false);
  let sent = alice.store(&team.server, |store| store.send_waiting()).await;
  sent.expect("the conflict is recorded on the version");
  let sending = workspace.db().waiting_publishes().expect("read");
  assert_eq!(
    sending
      .iter()
      .map(|publish| (publish.number, publish.state.clone()))
      .collect::<Vec<_>>(),
    [(number(3), PublishState::Conflict { latest: number(2) })],
    "the teammate's v2 took its number here"
  );
  let kept = latest::<AgentProfile>(workspace, builder_id).expect("a version used here");
  assert_eq!(
    (kept.id(), kept.body()),
    (published.id(), &own),
    "runs keep the owner's own version"
  );
  assert_eq!(
    waiting(&workspace.root, builder_id)
      .expect("read")
      .map(|waiting| waiting.number),
    Some(number(2)),
    "the teammate's widening v2 waits for Use v2 here"
  );

  let version = sending[0].version;
  alice
    .store(&team.server, move |store| store.publish_again(version))
    .await
    .expect("published as the next version");
  assert_eq!(workspace.db().waiting_publishes().expect("read"), []);
  assert_eq!(used(workspace, builder_id), Some(3));
  assert_eq!(
    waiting(&workspace.root, builder_id).expect("read"),
    None,
    "the owner's v3, numbered by the server, supersedes it"
  );
}

/// A team MCP server `docs` running `npx`, whose tools `exempt` run without asking.
fn docs_server(exempt: &[&str]) -> TeamMcpServer {
  serde_json::from_value(serde_json::json!({
    "name": "docs",
    "launch": {"kind": "program", "program": "npx", "args": ["-y"]},
    "exempt_tools": exempt,
  }))
  .expect("a team MCP server")
}

/// An MCP server is a team body that names nothing of a computer, so the owner publishes it from here.
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_version_published_here_and_one_that_does_not_widen_apply_at_once() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let workspace = &alice.workspace;
  let own = docs_server(&["search"]);
  let at = Timestamp::from_unix_millis(1);
  // The owner publishes it from here; the server numbers it at once.
  let docs_id = alice
    .store(&server, move |store| {
      let slug = Slug::new("docs").expect("slug");
      let created = store.create_resource(slug, Name::new("Docs").expect("name"), &own, at)?;
      store.publish_draft::<TeamMcpServer>(created.id(), at)?;
      Ok::<_, SharingError>(created.id())
    })
    .await
    .expect("v1 published here");
  assert_eq!(
    workspace.db().waiting_publishes().expect("read"),
    [],
    "the server numbered it"
  );
  let stream = workspace.follow(Member::of(&server, ALICE));
  seen_within(LIVE, "the resource the server's", || {
    matches!(
      workspace.db().resource_sharing(docs_id),
      Ok(Some(ResourceSharing::Shared(_)))
    )
  })
  .await;
  let used = |workspace: &Workspace| latest::<TeamMcpServer>(workspace, docs_id).map(|version| version.number().get());
  assert_eq!(used(workspace), Some(1), "published here, it never waits");
  assert_eq!(waiting(&workspace.root, docs_id).expect("read"), None);

  // A teammate narrows it: no tool runs without asking.
  server.publish(
    (docs_id, SharedResourceKind::McpServer, "Docs"),
    serde_json::to_value(docs_server(&[])).expect("a team MCP server serializes"),
    Vec::new(),
  );
  seen_within(LIVE, "v2 applied at once", || used(workspace) == Some(2)).await;
  assert_eq!(waiting(&workspace.root, docs_id).expect("read"), None);
  stream.abort();
}

/// A profile made here is created in the team's form: the CLI kind of its connection, and each MCP server it chooses
/// as the team MCP server it is. A server of this computer has no team counterpart, so a profile choosing one is not
/// published and the person is told which.
#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_profile_is_published_in_the_teams_form_and_one_choosing_a_local_mcp_server_is_not() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let docs = ResourceId::generate();
  server.publish(
    (docs, SharedResourceKind::McpServer, "Docs"),
    serde_json::to_value(docs_server(&[])).expect("a team MCP server serializes"),
    Vec::new(),
  );
  let alice = Computer::new(&server, ALICE, MemberRole::Owner);
  let workspace = &alice.workspace;
  let laptop = codex(&mut workspace.db(), &workspace.root, "Laptop");
  let stream = workspace.follow(Member::of(&server, ALICE));
  seen_within(LIVE, "the team MCP server", || {
    latest::<TeamMcpServer>(workspace, docs).is_some()
  })
  .await;
  stream.abort();

  let team_docs = ConnectionId::from_uuid(docs.as_uuid());
  let created = create_profile(
    &alice,
    &server,
    profile(laptop, &[team_docs], AgentPermissions::before_permissions()),
  )
  .await
  .expect("published in the team's form");
  let sent = server.versions_of(created).pop().expect("the server has v1");
  let wanted = team_profile(&[docs], AgentPermissions::before_permissions());
  assert_eq!(sent.body, serde_json::to_value(&wanted).expect("serializes"));
  assert_eq!(sent.references, ReferenceTarget::of_profile(&wanted));
  assert!(sent.body.get("connection").is_none(), "{}", sent.body);

  let notes = local_mcp_server();
  workspace.db().create_mcp_server(&notes).expect("a local MCP server");
  let refused = create_profile(
    &alice,
    &server,
    profile(laptop, &[notes.id()], AgentPermissions::before_permissions()),
  )
  .await;
  assert!(
    matches!(refused, Err(SharingError::LocalEntitiesNamed { ref names, .. }) if *names == ["notes"]),
    "{refused:?}"
  );
}

/// An MCP server that only this computer has.
fn local_mcp_server() -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("notes").expect("name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("notes-mcp").expect("program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::default(),
    Timestamp::from_unix_millis(1),
  )
  .expect("server")
}

/// Creates and publishes `profile` as a new team profile from `alice`'s computer.
async fn create_profile(
  alice: &Computer,
  server: &TeamServer,
  profile: AgentProfile,
) -> Result<ResourceId, SharingError> {
  let at = Timestamp::from_unix_millis(1);
  alice
    .store(server, move |store| {
      let slug = Slug::new(&format!("builder-{}", ResourceId::generate())).expect("slug");
      let created = store.create_resource(slug, Name::new("Builder").expect("name"), &profile, at)?;
      store.publish_draft::<AgentProfile>(created.id(), at)?;
      Ok::<_, SharingError>(created.id())
    })
    .await
}

#[test]
fn live_updates_no_mcp_tool_uses_a_version_here_or_binds_a_reference() {
  let names: Vec<String> = app_mcp::listed()
    .into_iter()
    .chain(app_mcp::setup::listed())
    .chain(tools::listed())
    .map(|tool| tool.name.to_string())
    .collect();
  assert!(!names.is_empty());
  for name in &names {
    assert!(
      !["bind", "use_here", "use_version"]
        .iter()
        .any(|word| name.contains(word)),
      "{name} reaches what only a person does"
    );
  }
}
