//! A teammate's version that widens what agents may do here (`docs/architecture/server.md`, row 35, "A teammate's
//! version that widens"): it is cached, but runs keep the version before it until the person's *Use v<N> here*, and
//! the resource says what it adds. No MCP tool and no automation presses *Use v<N> here* or binds an MCP server or a
//! secret. A version that does not widen, and one this computer published itself, apply at once.

use std::num::NonZeroU32;

use orchester_api::{Reference, ReferenceTarget, SharedResourceKind};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, widening::Widening},
  resource::{AgentProfile, Pipeline, RepositoryAccess, Slug, VersionNumber},
  run::{Answerer, PreflightProblem},
  sharing::VersionWidening,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{Bound, BoundBy, BoundReference, ResourceSharing};
use orchester_workspace::{
  app_mcp,
  orchestrator::tools,
  sharing::{
    SharingError,
    sync::{Waiting, bind, use_here, waiting},
  },
};

use super::{
  ALICE, LIVE, Member, TeamServer, Workspace,
  config::{agent, builder, codex, docs, key, latest, pipeline, problems, profile, reference, step, use_first},
  seen_within,
};

fn number(value: u32) -> VersionNumber {
  VersionNumber::new(NonZeroU32::new(value).expect("above zero"))
}

fn used(workspace: &Workspace, resource: ResourceId) -> Option<u32> {
  latest::<AgentProfile>(workspace, resource).map(|version| version.number().get())
}

/// Builder on the publisher's connection `theirs`, and pipeline Delivery that runs it.
struct Team {
  server: TeamServer,
  theirs: ConnectionId,
  builder: ResourceId,
  delivery: ResourceId,
}

impl Team {
  fn publish() -> Self {
    let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
    let theirs = ConnectionId::generate();
    let (builder_id, delivery) = (ResourceId::generate(), ResourceId::generate());
    server.publish(
      (builder_id, SharedResourceKind::AgentProfile, "Builder"),
      builder(theirs, &[]),
      vec![work(theirs)],
    );
    server.publish(
      (delivery, SharedResourceKind::Pipeline, "Delivery"),
      pipeline(vec![step("implement", agent(builder_id))]),
      Vec::new(),
    );
    Self {
      server,
      theirs,
      builder: builder_id,
      delivery,
    }
  }

  /// A workspace with a Codex connection and an MCP server `docs`, which follows the server until Builder's v1, which
  /// runs commands, waits; with the local server and the stream.
  async fn followed(&self) -> (Workspace, ConnectionId, tokio::task::JoinHandle<super::Stopped>) {
    let workspace = Workspace::linked(&self.server);
    let mut db = workspace.db();
    codex(&mut db, &workspace.root, "Laptop");
    let local_docs = docs(&mut db);
    drop(db);
    let stream = workspace.follow(Member::of(&self.server, ALICE));
    seen_within(LIVE, "the snapshot's resources", || {
      let builder_waits = workspace.db().waiting_version(self.builder).ok().flatten().is_some();
      latest::<Pipeline>(&workspace, self.delivery).is_some() && builder_waits
    })
    .await;
    (workspace, local_docs, stream)
  }
}

/// The reference of the publisher's Codex connection `theirs`.
fn work(theirs: ConnectionId) -> Reference {
  reference(ReferenceTarget::Connection { id: theirs }, "Work", Some("codex_cli"))
}

/// Neither *Use v2 here* nor binding the MCP server of `chosen` is automation's: both are refused and change nothing.
fn refused_to_automation(workspace: &Workspace, builder_id: ResourceId, chosen: BoundReference) {
  assert!(matches!(
    use_here(&workspace.root, builder_id, number(2), Answerer::Automation),
    Err(SharingError::PersonOnly)
  ));
  assert!(matches!(
    bind(&workspace.root, chosen, Answerer::Automation),
    Err(SharingError::PersonOnly)
  ));
  assert_eq!(used(workspace, builder_id), Some(1));
  let BoundReference::McpServer { reference, .. } = chosen else {
    panic!("an MCP server's binding");
  };
  let bindings = workspace.db().reference_bindings().expect("bindings");
  assert!(
    bindings
      .iter()
      .any(|kept| kept.reference == BoundReference::McpServer { reference, bound: None }),
    "{bindings:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_widening_version_waits_and_runs_keep_the_previous_one_until_use_here() {
  let team = Team::publish();
  let (builder_id, delivery) = (team.builder, team.delivery);
  let (workspace, local_docs, stream) = team.followed().await;
  assert_eq!(used(&workspace, builder_id), None, "no version is used here yet");
  assert_eq!(
    waiting(&workspace.root, builder_id).expect("read"),
    Some(Waiting {
      number: VersionNumber::FIRST,
      adds: vec![VersionWidening::Permissions(Widening::Commands)],
    }),
    "a first version that runs commands waits too"
  );
  use_first(&workspace, builder_id).await;
  assert_eq!(used(&workspace, builder_id), Some(1));
  assert_eq!(problems(&workspace, delivery), []);

  let their_docs = ConnectionId::generate();
  let wider = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  team.server.publish(
    (builder_id, SharedResourceKind::AgentProfile, "Builder"),
    serde_json::to_value(profile(team.theirs, &[their_docs], wider)).expect("a profile serializes"),
    vec![
      work(team.theirs),
      reference(ReferenceTarget::McpServer { id: their_docs }, "docs", None),
    ],
  );
  seen_within(LIVE, "v2 from its event", || {
    workspace.db().waiting_version(builder_id).ok().flatten().is_some()
  })
  .await;
  assert_eq!(used(&workspace, builder_id), Some(1), "runs keep v1");
  assert_eq!(
    problems(&workspace, delivery),
    [],
    "a run here takes v1, which needs no MCP server"
  );
  assert_eq!(
    waiting(&workspace.root, builder_id).expect("read"),
    Some(Waiting {
      number: number(2),
      adds: vec![
        VersionWidening::Permissions(Widening::Repository),
        VersionWidening::Permissions(Widening::Network),
        VersionWidening::McpServer,
      ],
    }),
    "the resource says what v2 adds"
  );
  let chosen = BoundReference::McpServer {
    reference: their_docs,
    bound: Some(Bound {
      local: local_docs,
      by: BoundBy::Person,
    }),
  };
  refused_to_automation(&workspace, builder_id, chosen);

  assert!(
    use_here(&workspace.root, builder_id, number(2), Answerer::Person).expect("used"),
    "the person's Use v2 here"
  );
  assert_eq!(used(&workspace, builder_id), Some(2), "runs take v2 now");
  assert_eq!(waiting(&workspace.root, builder_id).expect("read"), None);
  assert_eq!(
    problems(&workspace, delivery),
    [PreflightProblem::McpServerMissing { step: key("implement") }],
    "v2's MCP server is not bound here yet"
  );
  assert_eq!(
    bind(&workspace.root, chosen, Answerer::Person).expect("bound"),
    1,
    "the person binds it; v2 names it"
  );
  assert_eq!(problems(&workspace, delivery), []);
  stream.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_version_published_here_and_one_that_does_not_widen_apply_at_once() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let workspace = Workspace::linked(&server);
  let mut db = workspace.db();
  let laptop = codex(&mut db, &workspace.root, "Laptop");
  // Any command in the worktree: as a teammate's first version, this Builder would wait.
  let own = profile(laptop, &[], AgentPermissions::before_permissions());
  let at = Timestamp::from_unix_millis(1);
  let builder_id = db
    .create_resource(
      Slug::new("builder").expect("slug"),
      Name::new("Builder").expect("name"),
      &own,
      at,
    )
    .expect("created")
    .id();
  db.publish_draft::<AgentProfile>(builder_id, at)
    .expect("v1 published here");
  drop(db);
  // Sharing uploaded it: the server's v1 is this computer's.
  server.publish(
    (builder_id, SharedResourceKind::AgentProfile, "Builder"),
    serde_json::to_value(&own).expect("a profile serializes"),
    Vec::new(),
  );
  let stream = workspace.follow(Member::of(&server, ALICE));
  seen_within(LIVE, "the resource the server's", || {
    matches!(
      workspace.db().resource_sharing(builder_id),
      Ok(Some(ResourceSharing::Shared(_)))
    )
  })
  .await;
  assert_eq!(used(&workspace, builder_id), Some(1), "published here, it never waits");
  assert_eq!(waiting(&workspace.root, builder_id).expect("read"), None);

  // A teammate narrows it: no command, read-only, network off.
  let narrower = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  );
  server.publish(
    (builder_id, SharedResourceKind::AgentProfile, "Builder"),
    serde_json::to_value(profile(laptop, &[], narrower)).expect("a profile serializes"),
    vec![reference(
      ReferenceTarget::Connection { id: laptop },
      "Laptop",
      Some("codex_cli"),
    )],
  );
  seen_within(LIVE, "v2 applied at once", || used(&workspace, builder_id) == Some(2)).await;
  assert_eq!(waiting(&workspace.root, builder_id).expect("read"), None);
  stream.abort();
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
