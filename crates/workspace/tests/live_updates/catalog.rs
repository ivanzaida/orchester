//! This computer's catalog through the stream (`docs/architecture/server.md`, "Reporting", row 75): reported to the
//! fake server when the stream connects, again after a connection, an MCP server or a model list changed and not while
//! nothing did, and holding only kinds, names, roles and model ids: no path, program, version, effort or sign-in.
//!
//! `cargo test -p orchester-workspace --test live_updates catalog`

use std::{path::Path, time::Duration};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, LastCheck, ModelCatalog,
    SignIn,
    mcp::{McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
  },
  id::ConnectionId,
  name::Name,
  resource::{CommandArg, CommandProgram, ModelId},
  store::{ConnectionStore, McpServerStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::sharing::sync::{Clock, follow};
use serde_json::{Value, json};

use super::{ALICE, LIVE, Member, TeamServer, Workspace, seen_within};

/// How often the tests' streams read the catalog again.
const CHECK: Duration = Duration::from_millis(50);

/// Tokio's timer without jitter, reading the catalog every [`CHECK`].
struct Checking;

impl Clock for Checking {
  async fn wait(&self, wait: Duration) {
    tokio::time::sleep(wait).await;
  }

  fn jitter(&self) -> f64 {
    0.0
  }

  fn catalog_check(&self) -> Duration {
    CHECK
  }
}

fn at() -> Timestamp {
  Timestamp::from_unix_millis(1)
}

/// A signed-in check of version 0.139.0 that lists `models`, each with the efforts `low` and `high`.
fn check(models: &[&str]) -> ConnectionCheck {
  let models = models
    .iter()
    .map(|id| CatalogModel::new(ModelId::new(id).expect("a model id"), vec!["low".into(), "high".into()]))
    .collect();
  ConnectionCheck {
    at: at(),
    result: CheckResult::Available {
      version: "0.139.0".into(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, models),
    },
  }
}

/// A connection to `provider`'s CLI named `called`, its check listing `models`, kept in `db`.
fn connection(db: &mut ProjectDb, root: &Path, provider: AgentProvider, called: &str, models: &[&str]) -> ConnectionId {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(called).expect("a name"),
    provider,
    root.join("agent-cli.exe"),
    at(),
  )
  .expect("absolute path");
  connection
    .record_check(check(models))
    .expect("an agent check fits a CLI connection");
  let connection = connection.into_connection();
  db.create_connection(&connection).expect("kept");
  connection.id()
}

/// An MCP server named `called` that runs `secret-program --token`, kept in `db`.
fn mcp_server(db: &mut ProjectDb, called: &str) {
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(called).expect("a name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("secret-program").expect("a program"),
      vec![CommandArg::new("--token").expect("an argument")],
      None,
    )),
    McpEnvironment::default(),
    at(),
  )
  .expect("a server");
  db.create_mcp_server(&server).expect("kept");
}

fn followed(workspace: &Workspace, server: &TeamServer) -> tokio::task::JoinHandle<()> {
  let (root, member) = (workspace.root.clone(), Member::of(server, ALICE));
  tokio::spawn(async move {
    follow(&member, &root, &Checking).await;
  })
}

#[tokio::test(flavor = "multi_thread")]
async fn catalog_is_reported_on_connect_with_only_kinds_names_roles_and_model_ids() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &["Status bar"]);
  let workspace = Workspace::linked(&server);
  let mut db = workspace.db();
  let root = &workspace.root;
  connection(&mut db, root, AgentProvider::CodexCli, "Codex", &["gpt-5.5", "gpt-5.4"]);
  connection(
    &mut db,
    root,
    AgentProvider::ClaudeCodeCli,
    "Claude",
    &["claude-opus-5-5"],
  );
  connection(
    &mut db,
    root,
    AgentProvider::CodexCli,
    "Codex at home",
    &["gpt-5.5", "gpt-6"],
  );
  mcp_server(&mut db, "docs");
  drop(db);

  let stream = followed(&workspace, &server);
  seen_within(LIVE, "the report on connect", || !server.catalogs().is_empty()).await;
  let report = &server.catalogs()[0];
  assert_eq!(
    *report,
    json!({
      "references": [
        { "kind": "connection", "name": "Claude", "role": "claude_code_cli" },
        { "kind": "connection", "name": "Codex", "role": "codex_cli" },
        { "kind": "connection", "name": "Codex at home", "role": "codex_cli" },
        { "kind": "mcp_server", "name": "docs", "role": null },
      ],
      "models": {
        "claude_code_cli": ["claude-opus-5-5"],
        "codex_cli": ["gpt-5.4", "gpt-5.5", "gpt-6"],
      },
    }),
    "kinds, names and roles, and per role its connections' model ids, sorted and once each"
  );
  let text = report.to_string();
  for private in [
    "agent-cli",
    workspace.root.to_string_lossy().as_ref(),
    "secret-program",
    "--token",
    "0.139.0",
    "high",
    "signed_in",
  ] {
    assert!(!text.contains(private), "the report holds {private:?}: {text}");
  }
  stream.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn catalog_a_connection_mcp_server_or_model_list_change_is_reported_again() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &["Status bar"]);
  let workspace = Workspace::linked(&server);
  let codex = connection(
    &mut workspace.db(),
    &workspace.root,
    AgentProvider::CodexCli,
    "Codex",
    &["gpt-5.5"],
  );
  let stream = followed(&workspace, &server);
  seen_within(LIVE, "the report on connect", || server.catalogs().len() == 1).await;
  tokio::time::sleep(CHECK * 6).await;
  assert_eq!(server.catalogs().len(), 1, "nothing changed, nothing is sent again");

  mcp_server(&mut workspace.db(), "docs");
  seen_within(LIVE, "the report after an MCP server", || server.catalogs().len() == 2).await;
  assert_eq!(names(&server.catalogs()[1]), ["Codex", "docs"]);

  workspace
    .db()
    .record_connection_check(codex, LastCheck::Agent(check(&["gpt-5.5", "gpt-6"])))
    .expect("checked again");
  seen_within(LIVE, "the report after a model list", || server.catalogs().len() == 3).await;
  assert_eq!(
    server.catalogs()[2]["models"],
    json!({ "codex_cli": ["gpt-5.5", "gpt-6"] })
  );

  connection(
    &mut workspace.db(),
    &workspace.root,
    AgentProvider::ClaudeCodeCli,
    "Claude",
    &[],
  );
  seen_within(LIVE, "the report after a connection", || server.catalogs().len() == 4).await;
  assert_eq!(names(&server.catalogs()[3]), ["Claude", "Codex", "docs"]);
  assert_eq!(
    server.catalogs()[3]["models"],
    json!({ "codex_cli": ["gpt-5.5", "gpt-6"] }),
    "a role whose connections list no model has no models"
  );
  stream.abort();
}

/// The names a report's references carry, in its order.
fn names(report: &Value) -> Vec<&str> {
  report["references"]
    .as_array()
    .expect("references")
    .iter()
    .map(|reference| reference["name"].as_str().expect("a name"))
    .collect()
}
