//! The project's MCP servers in the project database. Run with `cargo test -p orchester-store --test
//! mcp_servers mcp`.

use std::{collections::BTreeMap, fs};

use orchester_core::{
  connection::mcp::{
    ExemptTools, HttpServer, McpEnvValue, McpEnvironment, McpServer, McpServerName, McpTransport, McpUrl, StdioServer,
    WorkingDirectory,
  },
  id::{ConnectionId, ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandArg, CommandProgram, VariableName},
  store::McpServerStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection as Sqlite;
use tempfile::TempDir;

fn project_repo() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn raw(repo: &TempDir) -> Sqlite {
  Sqlite::open(repo.path().join(".orchester").join("orchester.db")).expect("open the database")
}

fn name(value: &str) -> McpServerName {
  McpServerName::new(value).expect("valid name")
}

fn local(server: &str, token: SecretId) -> McpServer {
  let transport = McpTransport::Stdio(StdioServer::new(
    CommandProgram::new("npx").expect("valid program"),
    vec![
      CommandArg::new("-y").expect("valid"),
      CommandArg::new("@acme/docs-mcp").expect("valid"),
    ],
    Some(WorkingDirectory::new("tools").expect("valid")),
  ));
  let environment = McpEnvironment::new(BTreeMap::from([
    (
      VariableName::new("DOCS_TOKEN").expect("valid"),
      McpEnvValue::Secret(token),
    ),
    (
      VariableName::new("LOG_LEVEL").expect("valid"),
      McpEnvValue::Text("debug".to_owned()),
    ),
  ]))
  .expect("valid variables");
  McpServer::new(
    ConnectionId::generate(),
    name(server),
    transport,
    environment,
    Timestamp::from_unix_millis(1_000),
  )
  .expect("a valid server")
}

fn remote(server: &str) -> McpServer {
  let transport = McpTransport::Http(HttpServer::new(
    McpUrl::new("https://mcp.example.dev/v1").expect("valid"),
    Some(SecretId::generate()),
  ));
  McpServer::new(
    ConnectionId::generate(),
    name(server),
    transport,
    McpEnvironment::default(),
    Timestamp::from_unix_millis(2_000),
  )
  .expect("a valid server")
}

#[test]
fn mcp_servers_are_stored_and_read_back_in_name_order() {
  let (repo, mut db) = project_repo();
  let token = SecretId::generate();
  let (files, knowledge) = (local("files", token), remote("project-knowledge"));
  db.create_mcp_server(&knowledge).unwrap();
  db.create_mcp_server(&files).unwrap();
  drop(db);
  let db = ProjectDb::open(repo.path()).unwrap();
  assert_eq!(db.mcp_servers().unwrap(), vec![files.clone(), knowledge]);
  assert_eq!(db.mcp_server(files.id()).unwrap(), Some(files));
  assert_eq!(db.mcp_server(ConnectionId::generate()).unwrap(), None);
  let settings: String = raw(&repo)
    .query_row("SELECT settings FROM mcp_servers WHERE name = 'files'", [], |row| {
      row.get(0)
    })
    .unwrap();
  assert!(
    settings.contains(&token.to_string()),
    "a secret is stored as its id: {settings}"
  );
}

#[test]
fn mcp_server_names_are_unique_within_the_project() {
  let (_repo, mut db) = project_repo();
  db.create_mcp_server(&local("files", SecretId::generate())).unwrap();
  let error = db.create_mcp_server(&remote("files")).unwrap_err();
  assert!(matches!(error, StoreError::DuplicateConnectionName(name) if name == "files"));
  let mut other = remote("docs");
  db.create_mcp_server(&other).unwrap();
  other
    .update(name("files"), other.transport().clone(), McpEnvironment::default())
    .unwrap();
  assert!(matches!(
    db.save_mcp_server(&other),
    Err(StoreError::DuplicateConnectionName(_))
  ));
}

#[test]
fn mcp_servers_save_their_changes_and_enabled_flag() {
  let (repo, mut db) = project_repo();
  let mut server = local("files", SecretId::generate());
  db.create_mcp_server(&server).unwrap();
  let transport = McpTransport::Http(HttpServer::new(McpUrl::new("http://localhost:8931/mcp").unwrap(), None));
  server
    .update(name("files-http"), transport, McpEnvironment::default())
    .unwrap();
  server.set_enabled(false);
  db.save_mcp_server(&server).unwrap();
  drop(db);
  let db = ProjectDb::open(repo.path()).unwrap();
  let stored = db.mcp_server(server.id()).unwrap().expect("stored");
  assert_eq!(stored, server);
  assert!(!stored.enabled());
  let mut db = db;
  assert!(matches!(
    db.save_mcp_server(&remote("missing")),
    Err(StoreError::ConnectionNotFound(_))
  ));
}

#[test]
fn mcp_servers_revalidate_stored_rows_on_read() {
  let (repo, mut db) = project_repo();
  db.create_mcp_server(&local("files", SecretId::generate())).unwrap();
  raw(&repo)
    .execute(
      "UPDATE mcp_servers SET settings = replace(settings, '\"tools\"', '\"../outside\"')",
      [],
    )
    .unwrap();
  assert!(db.mcp_servers().unwrap().is_empty());
  let id = db.unreadable_mcp_servers().unwrap()[0].id.parse().expect("stored id");
  assert!(matches!(db.mcp_server(id), Err(StoreError::Corrupt { .. })));
  raw(&repo)
    .execute("UPDATE mcp_servers SET name = 'Not A Key'", [])
    .unwrap();
  assert!(matches!(db.mcp_server(id), Err(StoreError::Corrupt { .. })));
}

#[test]
fn mcp_servers_list_the_readable_rows_and_report_the_others() {
  let (repo, mut db) = project_repo();
  let files = local("files", SecretId::generate());
  db.create_mcp_server(&files).unwrap();
  db.create_mcp_server(&remote("knowledge")).unwrap();
  // A URL a later rule refuses: a backslash in the host.
  raw(&repo)
    .execute(
      r"UPDATE mcp_servers SET settings = replace(settings, 'mcp.example.dev', 'mcp.example.dev\\@evil.example') WHERE name = 'knowledge'",
      [],
    )
    .unwrap();
  assert_eq!(db.mcp_servers().unwrap(), vec![files]);
  let unreadable = db.unreadable_mcp_servers().unwrap();
  assert_eq!(unreadable.len(), 1);
  assert_eq!(unreadable[0].name, "knowledge");
  assert!(unreadable[0].detail.contains("backslash"), "{}", unreadable[0].detail);
}

#[test]
fn mcp_servers_keep_their_tools_that_run_without_asking() {
  let (repo, mut db) = project_repo();
  let mut server = remote("pencil");
  db.create_mcp_server(&server).unwrap();
  let settings = |repo: &TempDir| -> String {
    raw(repo)
      .query_row("SELECT settings FROM mcp_servers WHERE name = 'pencil'", [], |row| {
        row.get(0)
      })
      .unwrap()
  };
  assert!(
    !settings(&repo).contains("exempt_tools"),
    "a server without such tools stores them as before"
  );
  server.set_exempt_tools(
    ExemptTools::parse(
      "get_screenshot
batch_get
get_editor_state",
    )
    .unwrap(),
  );
  db.save_mcp_server(&server).unwrap();
  drop(db);
  let mut db = ProjectDb::open(repo.path()).unwrap();
  let stored = db.mcp_server(server.id()).unwrap().expect("stored");
  let names: Vec<&str> = stored.exempt_tools().tools().iter().map(|tool| tool.as_str()).collect();
  assert_eq!(
    names,
    ["get_screenshot", "batch_get", "get_editor_state"],
    "in the listed order"
  );
  assert_eq!(stored, server);
  raw(&repo)
    .execute(
      "UPDATE mcp_servers SET settings = replace(settings, 'batch_get', 'batch get')",
      [],
    )
    .unwrap();
  assert!(
    matches!(db.mcp_server(server.id()), Err(StoreError::Corrupt { .. })),
    "a stored tool name is checked again"
  );
  server.set_exempt_tools(ExemptTools::default());
  db.save_mcp_server(&server).unwrap();
  assert!(
    db.mcp_server(server.id())
      .unwrap()
      .expect("stored")
      .exempt_tools()
      .is_empty()
  );
}

#[test]
fn mcp_servers_are_deleted_by_id() {
  let (_repo, mut db) = project_repo();
  let files = local("files", SecretId::generate());
  let knowledge = remote("knowledge");
  db.create_mcp_server(&files).unwrap();
  db.create_mcp_server(&knowledge).unwrap();
  db.delete_mcp_server(files.id()).unwrap();
  assert_eq!(db.mcp_servers().unwrap(), vec![knowledge]);
  assert!(matches!(
    db.delete_mcp_server(files.id()),
    Err(StoreError::ConnectionNotFound(_))
  ));
}
