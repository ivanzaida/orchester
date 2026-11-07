//! Share uploads a local project's configuration in the team's form (`docs/design/team-configuration.md`, edge case
//! 12), against the fake team server: a connection becomes its CLI kind, each local MCP server an uploaded profile
//! chooses becomes a team MCP server with no program path and no value, and this computer's fill-ins are set from what
//! it had locally. What the team cannot take is refused whole, every resource listed, with nothing sent.
//!
//! `cargo test -p orchester-workspace --test live_updates live_updates_share_`

use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
};

use orchester_api::{ReferenceTarget, SharedResourceKind};
use orchester_core::{
  connection::mcp::{
    HttpServer, McpAuthorization, McpEnvValue, McpEnvironment, McpServer, McpServerName, McpServerSelection,
    McpTransport, McpUrl, StdioServer,
  },
  id::{ConnectionId, ResourceId, SecretId, ServerId},
  permission::{AgentPermissions, WritableFolders},
  resource::{AgentProfile, CommandProgram, Draft, TeamAgentProfile, TeamMcpLaunch, TeamMcpServer, VariableName},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::{McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb, ResourceSharing, StoredServer};
use orchester_workspace::{
  sharing::{
    ShareRefusalReason, SharingError, TeamMcpProblem,
    sync::{ShareFillIns, upload_configuration},
  },
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

/// A keychain in memory: tests never touch the keychain of the user running them.
#[derive(Default)]
struct MemoryVault(Mutex<BTreeMap<String, String>>);

fn key_text(key: &VaultKey) -> String {
  format!("{key:?}")
}

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key_text(key), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(&key_text(key)).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::NotFound)
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(&key_text(key));
    Ok(())
  }
}

const PROGRAM: &str = "/opt/tools/notes-mcp";
const TOKEN: &str = "s3cret-token-value";

fn secret(db: &mut ProjectDb, label: &str) -> Secret {
  let new = NewSecret {
    name: SecretName::new(label).expect("secret name"),
    kind: SecretKind::AccessToken,
    environment: Environment::Production,
  };
  let secret = Secret::new(SecretId::generate(), new, at(1));
  db.create_secret(&secret).expect("a local secret");
  secret
}

fn local_server(label: &str, transport: McpTransport, environment: McpEnvironment) -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(label).expect("server name"),
    transport,
    environment,
    at(2),
  )
  .expect("a server")
}

/// A stdio server `notes` started by an absolute path, with a plain variable and one that takes `token`.
fn notes(token: &Secret) -> McpServer {
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
  let stdio = StdioServer::new(CommandProgram::new(PROGRAM).expect("program"), Vec::new(), None);
  local_server(
    "notes",
    McpTransport::Stdio(stdio),
    McpEnvironment::new(variables).expect("variables"),
  )
}

fn remote(label: &str, url: &str, authorization: McpAuthorization) -> McpServer {
  let http = HttpServer::new(McpUrl::new(url).expect("url"), authorization);
  local_server(label, McpTransport::Http(http), McpEnvironment::default())
}

/// A published profile Reviewer, Builder's body choosing `servers`.
fn reviewer(alice: &mut Sharer, servers: &[ConnectionId]) -> ResourceId {
  let (_, builder) = alice.delivery();
  let published = alice.db().latest_version::<AgentProfile>(builder).expect("read");
  let body = published
    .expect("Builder")
    .body()
    .clone()
    .with_mcp_servers(McpServerSelection::new(servers.to_vec()).expect("servers"));
  profile_named(alice, "reviewer", &body)
}

fn profile_named(alice: &mut Sharer, label: &str, body: &AgentProfile) -> ResourceId {
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
  let local = notes(&token);
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
  assert!(sent.body.get("connection").is_none(), "{}", sent.body);

  let db = alice.db();
  assert!(matches!(
    db.resource_sharing(team_notes).expect("read"),
    Some(ResourceSharing::Shared(_))
  ));
  assert_eq!(
    db.latest_version::<TeamMcpServer>(team_notes)
      .expect("read")
      .expect("kept")
      .body(),
    &body
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
async fn live_updates_share_sets_this_computers_fill_ins_from_what_it_had_locally() {
  let team = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let token = secret(&mut alice.fixture.db, "NOTES_TOKEN");
  let local = notes(&token);
  alice.fixture.db.create_mcp_server(&local).expect("a local MCP server");
  let reviewer = reviewer(&mut alice, &[local.id()]);
  let (_, builder) = alice.delivery();
  let connection = alice
    .db()
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("Builder")
    .body()
    .connection();
  let vault = MemoryVault::default();
  let root = alice.fixture.repo.path().to_owned();
  vault
    .store(
      &token.vault_key(alice.db().project().id()),
      &SecretValue::new(TOKEN.to_owned()).expect("a value"),
    )
    .expect("the local value");
  let (workspace, (_folder, app_db), server) = workspace_with_server();

  let fill_ins = ShareFillIns::read(&root, &vault).expect("read before the upload");
  alice.share(&team).await;
  fill_ins.apply((&workspace, &vault), &root, server).expect("set");

  let db = alice.db();
  let pick = db.connection_pick(reviewer).expect("read").expect("a pick");
  assert_eq!(pick.connection, connection);
  assert!(db.connection_pick(builder).expect("read").is_some());
  let team_notes = ResourceId::from_uuid(local.id().as_uuid());
  assert!(db.team_mcp_on_here(team_notes).expect("read"), "it was on");
  let name = SecretName::new("NOTES_TOKEN").expect("name");
  let value = db.team_secret_value(&vault, &name).expect("read").expect("a value");
  assert_eq!(value.expose(), TOKEN);
  let program = AppDb::open(&app_db).expect("application database");
  let program = program.mcp_server_program(server, team_notes).expect("read");
  assert_eq!(program.expect("the path").program, Path::new(PROGRAM));
  let cached = db.cached_version::<AgentProfile>(reviewer, number(1)).expect("read");
  assert_eq!(
    cached.expect("cached").local_body.connection(),
    connection,
    "the sharer's profile still runs on its connection"
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
    .map(|header| header.as_str())
    .collect();
  assert_eq!(headers, ["Authorization"]);
  assert_eq!(
    on_server(server, unused),
    [],
    "a server no profile chooses is not uploaded"
  );
}

/// A workspace whose application database knows one server.
fn workspace_with_server() -> (Workspace, (TempDir, PathBuf), ServerId) {
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
