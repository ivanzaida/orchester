//! Share uploads a local project's configuration in the team's form (`docs/design/team-configuration.md`, edge case
//! 12), against the fake team server: a connection becomes its CLI kind, each local MCP server an uploaded profile
//! chooses becomes a team MCP server with no program path and no value, and this computer's fill-ins are set from what
//! it had locally. What the team cannot take is refused whole, every resource listed, with nothing sent.
//!
//! `cargo test -p orchester-workspace --test live_updates live_updates_share_`

use std::{
  collections::BTreeMap,
  path::PathBuf,
  sync::atomic::{AtomicBool, AtomicI64, Ordering},
};

use orchester_api::{ReferenceTarget, SharedResourceKind};
use orchester_core::{
  connection::mcp::{
    HttpServer, McpAuthorization, McpEnvValue, McpEnvironment, McpServer, McpServerName, McpServerSelection,
    McpTransport, McpUrl, StdioServer,
  },
  id::{ConnectionId, ResourceId, SecretId, ServerId},
  permission::{AgentPermissions, WritableFolders},
  resource::{
    AgentProfile, CommandProgram, Draft, HeaderPrefix, TeamAgentProfile, TeamMcpLaunch, TeamMcpServer, VariableName,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::{McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb, StoredServer};
use orchester_workspace::{
  sharing::{ShareRefusalReason, SharingError, TeamMcpProblem, sync::upload_configuration},
  workspace::Workspace,
};
use parking_lot::Mutex;
use tempfile::TempDir;

use super::{
  ALICE, TeamServer,
  publish::{Publisher, number, rule},
  share::{Sharer, name, on_server, slug},
  support::at,
};

/// A keychain in memory: tests never touch the keychain of the user running them. It can be told to refuse stores.
#[derive(Default)]
pub(super) struct MemoryVault {
  values: Mutex<BTreeMap<String, String>>,
  refuse_stores: AtomicBool,
}

impl MemoryVault {
  pub(super) fn refuse_stores(&self) {
    self.refuse_stores.store(true, Ordering::SeqCst);
  }
}

fn key_text(key: &VaultKey) -> String {
  format!("{key:?}")
}

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    if self.refuse_stores.load(Ordering::SeqCst) {
      return Err(VaultError::AccessDenied("refused by the test".to_owned()));
    }
    self.values.lock().insert(key_text(key), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self
      .values
      .lock()
      .get(&key_text(key))
      .cloned()
      .ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::NotFound)
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.values.lock().remove(&key_text(key));
    Ok(())
  }
}

static NEXT_SERVER: AtomicI64 = AtomicI64::new(0);

pub(super) const PROGRAM: &str = "/opt/tools/notes-mcp";
pub(super) const TOKEN: &str = "s3cret-token-value";

pub(super) fn secret(db: &mut ProjectDb, label: &str) -> Secret {
  let new = NewSecret {
    name: SecretName::new(label).expect("secret name"),
    kind: SecretKind::AccessToken,
    environment: Environment::Production,
  };
  let secret = Secret::new(SecretId::generate(), new, at(1));
  db.create_secret(&secret).expect("a local secret");
  secret
}

pub(super) fn local_server(label: &str, transport: McpTransport, environment: McpEnvironment) -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(label).expect("server name"),
    transport,
    environment,
    // In the order they are made: the plan reads servers by when they were made.
    at(2 + NEXT_SERVER.fetch_add(1, Ordering::SeqCst)),
  )
  .expect("a server")
}

/// A stdio server `notes` started by the absolute path `program`, with a plain variable and one that takes `token`.
pub(super) fn notes(token: &Secret, program: &str) -> McpServer {
  let variables = BTreeMap::from([
    (
      VariableName::new("API_TOKEN").expect("variable"),
      McpEnvValue::Secret(token.id()),
    ),
    (
      VariableName::new("MODE").expect("variable"),
      McpEnvValue::Text("fast".to_owned()),
    ),
  ]);
  let stdio = StdioServer::new(CommandProgram::new(program).expect("program"), Vec::new(), None);
  local_server(
    "notes",
    McpTransport::Stdio(stdio),
    McpEnvironment::new(variables).expect("variables"),
  )
}

pub(super) fn remote(label: &str, url: &str, authorization: McpAuthorization) -> McpServer {
  let http = HttpServer::new(McpUrl::new(url).expect("url"), authorization);
  local_server(label, McpTransport::Http(http), McpEnvironment::default())
}

/// A published profile Reviewer, Builder's body choosing `servers`.
pub(super) fn reviewer(alice: &mut Sharer, servers: &[ConnectionId]) -> ResourceId {
  let (_, builder) = alice.delivery();
  let published = alice.db().latest_version::<AgentProfile>(builder).expect("read");
  let body = published
    .expect("Builder")
    .body()
    .clone()
    .with_mcp_servers(McpServerSelection::new(servers.to_vec()).expect("servers"));
  profile_named(alice, "reviewer", &body)
}

pub(super) fn profile_named(alice: &mut Sharer, label: &str, body: &AgentProfile) -> ResourceId {
  let db = &mut alice.fixture.db;
  let id = db
    .create_resource(slug(label), name(label), body, at(30))
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, at(31)).expect("v1");
  id
}

fn json_text(value: &serde_json::Value) -> String {
  value.to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_turns_the_local_mcp_servers_a_profile_chooses_into_team_mcp_servers() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let token = secret(&mut alice.fixture.db, "NOTES_TOKEN");
  let local = notes(&token, PROGRAM);
  let issues = remote(
    "issues",
    "https://mcp.example.dev/issues",
    McpAuthorization::Bearer { token: token.id() },
  );
  let unused = remote("unused", "https://mcp.example.dev/unused", McpAuthorization::None);
  for kept in [&local, &issues, &unused] {
    alice.fixture.db.create_mcp_server(kept).expect("a local MCP server");
  }
  let reviewer = reviewer(&mut alice, &[local.id(), issues.id()]);

  alice.share(&server).await;

  let (team_notes, team_issues) = (
    ResourceId::from_uuid(local.id().as_uuid()),
    ResourceId::from_uuid(issues.id().as_uuid()),
  );
  let sent = server.versions_of(team_notes);
  assert_eq!(sent.len(), 1, "one version");
  assert_eq!(sent[0].number.get(), 1);
  assert!(sent[0].references.is_empty());
  let body: TeamMcpServer = serde_json::from_value(sent[0].body.clone()).expect("a team MCP server");
  let TeamMcpLaunch::Program(program) = body.launch() else {
    panic!("a program: {body:?}");
  };
  assert_eq!(program.program().as_str(), "notes-mcp", "the name, not the path");
  let mut variables: Vec<_> = program.variables().iter().map(VariableName::as_str).collect();
  variables.sort_unstable();
  assert_eq!(variables, ["API_TOKEN", "MODE"], "the names, no value");
  let text = json_text(&sent[0].body);
  for leaked in [
    "/opt/tools",
    "fast",
    &token.id().to_string(),
    &local.id().to_string(),
    TOKEN,
  ] {
    assert!(!text.contains(leaked), "{leaked} reached the team: {text}");
  }

  assert_remote_server(&server, team_issues, ResourceId::from_uuid(unused.id().as_uuid()));

  let sent = server.versions_of(reviewer).pop().expect("Reviewer v1");
  let team: TeamAgentProfile = serde_json::from_value(sent.body.clone()).expect("a team profile");
  assert_eq!(team.mcp_servers(), [team_notes, team_issues]);
  assert!(
    sent.references.contains(&ReferenceTarget::TeamResource {
      kind: SharedResourceKind::McpServer,
      id: team_notes
    }),
    "{:?}",
    sent.references
  );
  // The team servers take the local servers' ids on purpose, so those ids appear; the secret's id and values do not.
  assert!(sent.body.get("connection").is_none(), "{}", sent.body);
  assert_profile_leaks_nothing(&alice, reviewer, &sent, &token);

  let db = alice.db();
  assert!(
    matches!(
      db.resource_sharing(team_notes),
      Err(orchester_store::StoreError::ResourceNotFound(_))
    ),
    "the team MCP server is the team's on the server; nothing of it is kept here"
  );
  assert_eq!(db.mcp_servers().expect("read").len(), 3, "the local servers stay");
  let local_profile = db.latest_version::<AgentProfile>(reviewer).expect("read").expect("v1");
  assert_eq!(
    local_profile.body().mcp_servers().ids(),
    [local.id(), issues.id()],
    "the local profile still chooses the local servers, which have the team servers' ids"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_refuses_every_resource_the_team_cannot_take_and_sends_nothing() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let (delivery, builder) = alice.delivery();
  let folders: WritableFolders = serde_json::from_value(serde_json::json!(["/opt/cache"])).expect("folders");
  let published = alice.db().latest_version::<AgentProfile>(builder).expect("read");
  let body = published
    .expect("Builder")
    .body()
    .clone()
    .with_permissions(AgentPermissions::before_permissions().with_writable_folders(folders));
  let cache = profile_named(&mut alice, "cache-writer", &body);
  let token = secret(&mut alice.fixture.db, "NOTES_TOKEN");
  let tone = alice
    .fixture
    .db
    .create_resource(slug("tone"), name("Tone"), &rule(&token.id().to_string()), at(32))
    .expect("Tone")
    .id();
  alice
    .fixture
    .db
    .publish_draft::<orchester_core::resource::Instruction>(tone, at(33))
    .expect("v1");
  let publisher = Publisher::online(&server, ALICE);

  let refused = upload_configuration(&publisher, alice.fixture.repo.path()).await;

  let Err(SharingError::ShareRefused { refusals }) = &refused else {
    panic!("{refused:?}");
  };
  let named: Vec<_> = refusals.iter().map(|refusal| refusal.name.as_str()).collect();
  assert_eq!(named, ["Tone", "cache-writer"], "{refusals:?}");
  assert!(matches!(
    &refusals[1].reason,
    ShareRefusalReason::WritableFolders { folders } if *folders == ["/opt/cache"]
  ));
  assert!(matches!(&refusals[0].reason, ShareRefusalReason::LocalEntities { names } if *names == ["NOTES_TOKEN"]));
  assert_eq!(refusals[1].resource, cache);
  assert_eq!(refusals[0].resource, tone);
  for resource in [delivery, builder, cache, tone] {
    assert_eq!(on_server(&server, resource), [], "nothing was uploaded");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_refuses_a_local_mcp_server_the_team_cannot_run() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let token = secret(&mut alice.fixture.db, "NOTES_TOKEN");
  let in_tree = local_server(
    "scripts",
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("tools/serve.sh").expect("program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::default(),
  );
  let keyed = remote("keyed", "https://mcp.example.dev/mcp?key=abc", McpAuthorization::None);
  let basic = remote(
    "basic",
    "https://mcp.example.dev/basic",
    McpAuthorization::Basic {
      username: orchester_core::connection::mcp::BasicUsername::new("bob").expect("user"),
      password: token.id(),
    },
  );
  for kept in [&in_tree, &keyed, &basic] {
    alice.fixture.db.create_mcp_server(kept).expect("a local MCP server");
  }
  reviewer(&mut alice, &[in_tree.id(), keyed.id(), basic.id()]);
  let publisher = Publisher::online(&server, ALICE);

  let refused = upload_configuration(&publisher, alice.fixture.repo.path()).await;

  let Err(SharingError::ShareRefused { refusals }) = &refused else {
    panic!("{refused:?}");
  };
  let problems: Vec<_> = refusals
    .iter()
    .map(|refusal| match &refusal.reason {
      ShareRefusalReason::McpServer { server, problem } => (server.as_str(), problem.clone()),
      other => panic!("{other:?}"),
    })
    .collect();
  assert_eq!(
    problems,
    [
      (
        "scripts",
        TeamMcpProblem::ProgramInWorktree {
          program: "tools/serve.sh".to_owned()
        }
      ),
      ("keyed", TeamMcpProblem::UrlQuery),
      ("basic", TeamMcpProblem::BasicUsername),
    ]
  );
  assert!(
    !format!("{refusals:?}").contains("abc"),
    "the key in the address is not repeated"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_leaves_a_gone_mcp_server_out_of_an_older_version_and_refuses_it_in_the_newest() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let (_, builder) = alice.delivery();
  let published = alice.db().latest_version::<AgentProfile>(builder).expect("read");
  let base = published.expect("Builder").body().clone();
  let gone = ConnectionId::generate();
  let choosing = base
    .clone()
    .with_mcp_servers(McpServerSelection::new(vec![gone]).expect("one server"));
  let history = profile_named(&mut alice, "history", &choosing);
  let db = &mut alice.fixture.db;
  db.save_draft(history, &Draft::new(base.clone(), Some(number(1)), at(40)))
    .expect("a draft of v2");
  db.publish_draft::<AgentProfile>(history, at(41)).expect("v2");
  let publisher = Publisher::online(&server, ALICE);

  let uploaded = upload_configuration(&publisher, alice.fixture.repo.path()).await;

  assert!(uploaded.is_ok(), "{uploaded:?}");
  let versions = server.versions_of(history);
  assert_eq!(versions.len(), 2);
  for version in &versions {
    let team: TeamAgentProfile = serde_json::from_value(version.body.clone()).expect("a team profile");
    assert!(team.mcp_servers().is_empty());
  }

  let db = &mut alice.fixture.db;
  let newest = base.with_mcp_servers(McpServerSelection::new(vec![gone]).expect("one server"));
  db.save_draft(history, &Draft::new(newest, Some(number(2)), at(50)))
    .expect("a draft of v3");
  db.publish_draft::<AgentProfile>(history, at(51)).expect("v3");
  let refused = upload_configuration(&publisher, alice.fixture.repo.path()).await;
  let Err(SharingError::ShareRefused { refusals }) = &refused else {
    panic!("{refused:?}");
  };
  assert!(
    matches!(&refusals[0].reason, ShareRefusalReason::LocalEntities { .. }),
    "{refusals:?}"
  );
}

/// The profile as the team holds it, and its references, name nothing of this computer: not its connection, its
/// program path, a plain value, or a secret's id and value.
fn assert_profile_leaks_nothing(
  alice: &Sharer,
  profile: ResourceId,
  sent: &orchester_api::SharedVersion,
  token: &Secret,
) {
  let text = format!("{} {:?}", json_text(&sent.body), sent.references);
  let connection = alice
    .db()
    .latest_version::<AgentProfile>(profile)
    .expect("read")
    .expect("v1")
    .body()
    .connection();
  for leaked in [&connection.to_string(), PROGRAM, "fast", TOKEN, &token.id().to_string()] {
    assert!(!text.contains(leaked), "{leaked} reached the team: {text}");
  }
}

/// The remote server `issues` was uploaded with its bearer token's header, and `unused` was not uploaded.
fn assert_remote_server(server: &TeamServer, issues: ResourceId, unused: ResourceId) {
  let remote_body: TeamMcpServer =
    serde_json::from_value(server.versions_of(issues)[0].body.clone()).expect("a team MCP server");
  let TeamMcpLaunch::Remote(remote_launch) = remote_body.launch() else {
    panic!("a remote server");
  };
  let headers: Vec<_> = remote_launch
    .secret_headers()
    .iter()
    .map(|header| {
      (
        header.header().as_str(),
        header.secret().as_str(),
        header.prefix().map(HeaderPrefix::as_str),
      )
    })
    .collect();
  assert_eq!(headers, [("Authorization", "NOTES_TOKEN", Some("Bearer "))]);
  assert_eq!(
    on_server(server, unused),
    [],
    "a server no profile chooses is not uploaded"
  );
}

/// A workspace whose application database knows one server.
pub(super) fn workspace_with_server() -> (Workspace, (TempDir, PathBuf), ServerId) {
  let folder = tempfile::tempdir().expect("application data");
  let path = folder.path().join("orchester.db");
  let mut app = AppDb::open(&path).expect("application database");
  let server = ServerId::generate();
  let stored = StoredServer {
    id: server,
    url: "https://team.test".to_owned(),
    name: "Team".to_owned(),
    plain_http_allowed: false,
    certificate_pin: None,
    added_at: Timestamp::from_unix_millis(1),
    joined_with: None,
    account: None,
  };
  app.add_server(&stored, None).expect("server");
  (Workspace::new(app), (folder, path), server)
}
