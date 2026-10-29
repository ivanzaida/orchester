//! The team's configuration through the stream (`docs/architecture/server.md`, "Shared configuration"): resources the
//! snapshot names and versions published later reach the workspace's cache from the fake server; a connection
//! reference binds by its role, an MCP server or secret reference stays unbound and preflight refuses a run that needs
//! it, and a person's binding writes the local bodies again while the server's stay as published.

use std::{
  collections::{BTreeMap, BTreeSet},
  path::Path,
};

use orchester_api::{Reference, ReferenceTarget, SharedResourceKind};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
    mcp::{McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer},
  },
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AllowedSteps, ApprovalMode, CommandProgram, DatabaseQuery,
    DeniedBehavior, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, ResourceBody, Step, StepKey,
    StepKind, Tool, ToolAction, Version, VersionNumber,
  },
  run::PreflightProblem,
  store::{ConnectionStore, McpServerStore, ResourceStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::{Bound, BoundBy, BoundReference, ProjectDb, ReferenceBinding};
use orchester_workspace::{sharing::sync::bind, tasks::run::check};
use serde_json::Value;

use super::{ALICE, LIVE, Member, TeamServer, Workspace, seen_within};

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn at() -> Timestamp {
  Timestamp::from_unix_millis(1)
}

/// A signed-in Codex connection named `called`, kept in `db`.
fn codex(db: &mut ProjectDb, root: &Path, called: &str) -> ConnectionId {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name(called),
    AgentProvider::CodexCli,
    root.join("codex.exe"),
    at(),
  )
  .expect("absolute path");
  connection
    .record_check(ConnectionCheck {
      at: at(),
      result: CheckResult::Available {
        version: "0.139.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("an agent check fits a CLI connection");
  let connection = connection.into_connection();
  db.create_connection(&connection).expect("kept");
  connection.id()
}

/// An MCP server named `docs`, kept in `db`.
fn docs(db: &mut ProjectDb) -> ConnectionId {
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("docs").expect("valid name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("docs-mcp").expect("valid program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::default(),
    at(),
  )
  .expect("valid server");
  db.create_mcp_server(&server).expect("kept");
  server.id()
}

/// The publisher's Builder profile, on their connection `connection`, with their MCP servers `servers`.
fn builder(connection: ConnectionId, servers: &[ConnectionId]) -> Value {
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("valid model"),
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(servers.to_vec()).expect("servers"));
  serde_json::to_value(profile).expect("a profile serializes")
}

fn step(step: &str, kind: StepKind) -> Step {
  Step {
    key: key(step),
    name: name(step),
    kind,
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn agent(profile: ResourceId) -> StepKind {
  StepKind::Agent {
    profile: Some(profile),
    repository: RepositoryAccess::Write,
    model: None,
    permissions: StepPermissions::default(),
  }
}

fn pipeline(steps: Vec<Step>) -> Value {
  serde_json::to_value(Pipeline::new(steps).expect("pipeline")).expect("a pipeline serializes")
}

/// A database tool connecting with the publisher's secret `credentials`, which pipeline `delivery` may call.
fn lookup(credentials: SecretId, delivery: ResourceId) -> Value {
  let access = AccessPolicy::new(
    BTreeMap::from([(delivery, AllowedSteps::All)]),
    name("Staging"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("valid policy");
  let query = DatabaseQuery::new(credentials, "SELECT 1".into()).expect("query");
  let tool = Tool::new("", ToolAction::DatabaseQuery(query), Some(access)).expect("valid tool");
  serde_json::to_value(tool).expect("a tool serializes")
}

fn reference(target: ReferenceTarget, called: &str, role: Option<&str>) -> Reference {
  Reference {
    target,
    name: called.to_owned(),
    role: role.map(str::to_owned),
  }
}

fn latest<B: ResourceBody>(workspace: &Workspace, resource: ResourceId) -> Option<Version<B>> {
  // The resource is not in the cache until its first version is read.
  workspace.db().latest_version::<B>(resource).ok().flatten()
}

/// What preflight says of a run of `delivery`'s v1 here.
fn problems(workspace: &Workspace, delivery: ResourceId) -> Vec<PreflightProblem> {
  let details = TaskDetails {
    title: name("Search"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: Some(delivery),
    limits: TaskLimits::NONE,
  };
  check(&workspace.db(), &details, VersionNumber::FIRST).expect("checked")
}

fn kept_binding(workspace: &Workspace, reference: BoundReference) -> ReferenceBinding {
  let target = |kept: &BoundReference| match kept {
    BoundReference::Connection { reference, .. } | BoundReference::McpServer { reference, .. } => reference.to_string(),
    BoundReference::Secret { reference, .. } => reference.to_string(),
  };
  workspace
    .db()
    .reference_bindings()
    .expect("bindings")
    .into_iter()
    .find(|kept| target(&kept.reference) == target(&reference))
    .expect("a binding of the reference")
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_profile_published_on_the_server_runs_here_with_its_connection_bound_by_role() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let theirs = ConnectionId::generate();
  let (profile, delivery) = (ResourceId::generate(), ResourceId::generate());
  let work = reference(ReferenceTarget::Connection { id: theirs }, "Work", Some("codex_cli"));
  server.publish(
    (profile, SharedResourceKind::AgentProfile, "Builder"),
    builder(theirs, &[]),
    vec![work.clone()],
  );
  server.publish(
    (delivery, SharedResourceKind::Pipeline, "Delivery"),
    pipeline(vec![step("implement", agent(profile))]),
    Vec::new(),
  );
  let workspace = Workspace::linked(&server);
  let laptop = codex(&mut workspace.db(), &workspace.root, "Laptop");
  let stream = workspace.follow(Member::of(&server, ALICE));
  seen_within(LIVE, "the snapshot's resources", || {
    latest::<Pipeline>(&workspace, delivery).is_some()
  })
  .await;

  let kept = workspace
    .db()
    .cached_version::<AgentProfile>(profile, VersionNumber::FIRST)
    .expect("read")
    .expect("cached from the server");
  assert_eq!(kept.local_body.connection(), laptop, "the only Codex connection here");
  assert_eq!(kept.server_body, builder(theirs, &[]), "the server's body as published");
  assert_eq!(kept.publisher.name, "Alice Moreau");
  assert_eq!(
    kept_binding(
      &workspace,
      BoundReference::Connection {
        reference: theirs,
        bound: None
      }
    )
    .reference,
    BoundReference::Connection {
      reference: theirs,
      bound: Some(Bound {
        local: laptop,
        by: BoundBy::Role,
      }),
    }
  );
  assert_eq!(problems(&workspace, delivery), [], "the pipeline runs here");

  server.publish(
    (profile, SharedResourceKind::AgentProfile, "Builder"),
    builder(theirs, &[]),
    vec![work],
  );
  seen_within(LIVE, "v2 from its event", || {
    latest::<AgentProfile>(&workspace, profile).is_some_and(|version| version.number().get() == 2)
  })
  .await;
  let v2 = latest::<AgentProfile>(&workspace, profile).expect("v2");
  assert_eq!(v2.body().connection(), laptop, "a later version reuses the binding");
  seen_within(LIVE, "the cursor at the last event", || {
    workspace.link().cursor == server.cursor()
  })
  .await;
  stream.abort();
}

/// Profile Builder gives its agents the publisher's MCP server `docs`; tool Lookup connects with their secret.
struct Unbound {
  server: TeamServer,
  theirs: ConnectionId,
  their_docs: ConnectionId,
  profile: ResourceId,
  delivery: ResourceId,
}

impl Unbound {
  fn publish() -> Self {
    let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
    let (theirs, their_docs, token) = (ConnectionId::generate(), ConnectionId::generate(), SecretId::generate());
    let (profile, tool, delivery) = (ResourceId::generate(), ResourceId::generate(), ResourceId::generate());
    server.publish(
      (profile, SharedResourceKind::AgentProfile, "Builder"),
      builder(theirs, &[their_docs]),
      vec![
        reference(ReferenceTarget::Connection { id: theirs }, "Work", Some("codex_cli")),
        reference(ReferenceTarget::McpServer { id: their_docs }, "docs", None),
      ],
    );
    server.publish(
      (tool, SharedResourceKind::Tool, "Lookup"),
      lookup(token, delivery),
      vec![reference(
        ReferenceTarget::Secret { id: token },
        "Read-only database",
        Some("database_connection"),
      )],
    );
    server.publish(
      (delivery, SharedResourceKind::Pipeline, "Delivery"),
      pipeline(vec![
        step("implement", agent(profile)),
        step("checks", StepKind::Tool { tool: Some(tool) }),
      ]),
      Vec::new(),
    );
    Self {
      server,
      theirs,
      their_docs,
      profile,
      delivery,
    }
  }

  /// A workspace with a Codex connection named as the publisher's and an MCP server of the same name as theirs, which
  /// follows the server until its configuration is cached; with the stream that follows it.
  async fn cached(
    &self,
  ) -> (
    Workspace,
    ConnectionId,
    ConnectionId,
    tokio::task::JoinHandle<super::Stopped>,
  ) {
    let workspace = Workspace::linked(&self.server);
    let mut db = workspace.db();
    let (work, docs) = (codex(&mut db, &workspace.root, "work"), docs(&mut db));
    drop(db);
    let stream = workspace.follow(Member::of(&self.server, ALICE));
    seen_within(LIVE, "the snapshot's resources", || {
      latest::<Pipeline>(&workspace, self.delivery).is_some()
    })
    .await;
    (workspace, work, docs, stream)
  }

  fn servers_here(&self, workspace: &Workspace) -> Vec<ConnectionId> {
    let version = latest::<AgentProfile>(workspace, self.profile).expect("cached");
    version.body().mcp_servers().ids().to_vec()
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_an_mcp_server_or_secret_reference_stays_unbound_and_its_run_fails_preflight() {
  let unbound = Unbound::publish();
  let (workspace, work, _docs, stream) = unbound.cached().await;
  let bindings = workspace.db().reference_bindings().expect("bindings");
  let bound: Vec<_> = bindings.iter().map(|kept| kept.reference).collect();
  assert!(
    bound.contains(&BoundReference::Connection {
      reference: unbound.theirs,
      bound: Some(Bound {
        local: work,
        by: BoundBy::Name
      }),
    }),
    "the connection by its name, without regard to case: {bound:?}"
  );
  assert!(bound.contains(&BoundReference::McpServer {
    reference: unbound.their_docs,
    bound: None,
  }));
  assert!(
    bound
      .iter()
      .any(|kept| matches!(kept, BoundReference::Secret { bound: None, .. })),
    "{bound:?}"
  );
  assert_eq!(
    unbound.servers_here(&workspace),
    [unbound.their_docs],
    "a server of the same name here is not bound without a person"
  );
  let problems = problems(&workspace, unbound.delivery);
  assert!(
    problems.contains(&PreflightProblem::McpServerMissing { step: key("implement") }),
    "{problems:?}"
  );
  assert!(
    problems.contains(&PreflightProblem::SecretMissing { step: key("checks") }),
    "{problems:?}"
  );
  stream.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_binding_change_rewrites_the_local_bodies() {
  let unbound = Unbound::publish();
  let (workspace, work, docs, stream) = unbound.cached().await;
  let chosen = BoundReference::McpServer {
    reference: unbound.their_docs,
    bound: Some(Bound {
      local: docs,
      by: BoundBy::Name,
    }),
  };
  assert_eq!(
    bind(&workspace.root, chosen).expect("bound"),
    1,
    "Builder's v1 names it"
  );
  assert_eq!(
    kept_binding(&workspace, chosen).reference,
    BoundReference::McpServer {
      reference: unbound.their_docs,
      bound: Some(Bound {
        local: docs,
        by: BoundBy::Person
      }),
    },
    "a person bound it, whatever the choice said"
  );
  assert_eq!(unbound.servers_here(&workspace), [docs]);
  let kept = workspace
    .db()
    .cached_version::<AgentProfile>(unbound.profile, VersionNumber::FIRST)
    .expect("read")
    .expect("cached");
  assert_eq!(
    kept.server_body,
    builder(unbound.theirs, &[unbound.their_docs]),
    "the server's body stays"
  );
  assert_eq!(
    kept.local_body.connection(),
    work,
    "the other bindings stay in the body"
  );
  let problems = problems(&workspace, unbound.delivery);
  assert!(
    !problems.contains(&PreflightProblem::McpServerMissing { step: key("implement") }),
    "{problems:?}"
  );

  let spare = codex(&mut workspace.db(), &workspace.root, "Spare");
  let rebound = BoundReference::Connection {
    reference: unbound.theirs,
    bound: Some(Bound {
      local: spare,
      by: BoundBy::Person,
    }),
  };
  assert_eq!(bind(&workspace.root, rebound).expect("rebound"), 1);
  let profile = latest::<AgentProfile>(&workspace, unbound.profile).expect("cached");
  assert_eq!(profile.body().connection(), spare);

  let unchosen = BoundReference::McpServer {
    reference: unbound.their_docs,
    bound: None,
  };
  assert_eq!(bind(&workspace.root, unchosen).expect("unbound"), 1);
  assert_eq!(
    unbound.servers_here(&workspace),
    [unbound.their_docs],
    "unbound again: the publisher's id"
  );
  stream.abort();
}
