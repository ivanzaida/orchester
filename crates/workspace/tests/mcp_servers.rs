//! MCP servers as the Connections screen edits them: the dialog's fields to a server and back, a
//! variable taken from a secret written by the secret's name, the tools that run without asking, and
//! the profiles that give a server to their agents. Run with `cargo test -p orchester-desktop --test mcp_servers mcp`.

use std::fs;

use orchester_core::{
  connection::mcp::{
    ExemptTools, HttpServer, McpEnvValue, McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport,
    McpUrl,
  },
  id::{ConnectionId, ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, Slug},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_desktop::{
  connections::{
    data::ProjectConnections,
    mcp::{McpServerText, TransportChoice, parse_variables, save_in, secret_reference, set_enabled_in},
  },
  pipelines::data::Assignables,
};
use orchester_store::ProjectDb;
use tempfile::TempDir;

fn project_db() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create the project database");
  (repo, db)
}

fn secret(name: &str, environment: Environment) -> Secret {
  let new = NewSecret {
    name: SecretName::new(name).expect("valid name"),
    kind: SecretKind::AccessToken,
    environment,
  };
  Secret::new(SecretId::generate(), new, Timestamp::from_unix_millis(1))
}

fn stdio_text() -> McpServerText {
  McpServerText {
    name: "docs".to_owned(),
    program: "npx".to_owned(),
    arguments: "-y\n@acme/docs-mcp\n".to_owned(),
    working_directory: "tools".to_owned(),
    variables: "LOG_LEVEL=debug\nDOCS_TOKEN=${secret:DOCS_TOKEN@staging}".to_owned(),
    ..McpServerText::new_server()
  }
}

#[test]
fn mcp_dialog_fields_become_a_server_and_read_back() {
  let secrets = vec![
    secret("DOCS_TOKEN", Environment::Staging),
    secret("DOCS_TOKEN", Environment::Production),
  ];
  let draft = stdio_text().draft(&[], &secrets).expect("valid fields");
  let McpTransport::Stdio(stdio) = &draft.transport else {
    panic!("a program the CLI starts");
  };
  assert_eq!(stdio.program().as_str(), "npx");
  assert_eq!(stdio.args().len(), 2);
  let values: Vec<&McpEnvValue> = draft.environment.iter().map(|(_, value)| value).collect();
  assert_eq!(
    values,
    [
      &McpEnvValue::Secret(secrets[0].id()),
      &McpEnvValue::Text("debug".to_owned())
    ]
  );
  let server = McpServer::new(
    ConnectionId::generate(),
    draft.name,
    draft.transport,
    draft.environment,
    Timestamp::from_unix_millis(1),
  )
  .expect("a valid server");
  let text = McpServerText::of(&server, &secrets);
  assert_eq!(
    text.variables,
    format!("DOCS_TOKEN={}\nLOG_LEVEL=debug", secret_reference(&secrets[0]))
  );
  assert_eq!(text.arguments, "-y\n@acme/docs-mcp");
  // A secret the project no longer lists keeps its reference.
  let kept = McpServerText::of(&server, &[]);
  assert!(kept.variables.contains(&secrets[0].id().to_string()));
  assert_eq!(
    parse_variables(&kept.variables, &[]).expect("still valid"),
    *server.environment()
  );
}

#[test]
fn mcp_dialog_says_why_a_server_cannot_be_saved() {
  let taken = [McpServerName::new("docs").expect("valid")];
  let errors = stdio_text()
    .draft(&taken, &[])
    .expect_err("taken name and unknown secret");
  assert_eq!(
    errors.name.map(|message| message.key),
    Some("errors.connectionNameTaken")
  );
  assert_eq!(
    errors.variables.map(|message| message.key),
    Some("connections.mcp.errors.unknownSecret")
  );
  let remote = McpServerText {
    name: "Knowledge Base".to_owned(),
    transport: TransportChoice::Http,
    url: "https://user:pw@mcp.example.dev".to_owned(),
    ..McpServerText::new_server()
  };
  let errors = remote.draft(&[], &[]).expect_err("invalid name and URL");
  assert_eq!(
    errors.name.map(|message| message.key),
    Some("connections.mcp.errors.nameInvalid")
  );
  assert_eq!(
    errors.url.map(|message| message.key),
    Some("connections.mcp.errors.urlCredentials")
  );
  let folder = McpServerText {
    working_directory: "../outside".to_owned(),
    variables: String::new(),
    ..stdio_text()
  };
  let errors = folder.draft(&[], &[]).expect_err("a folder outside the worktree");
  assert_eq!(
    errors.working_directory.map(|message| message.key),
    Some("connections.mcp.errors.workingDirectory")
  );
}

#[test]
fn mcp_servers_are_saved_listed_and_turned_off() {
  let (repo, mut db) = project_db();
  let token = secret("DOCS_TOKEN", Environment::Staging);
  db.create_secret(&token).expect("secret");
  let draft = stdio_text().draft(&[], std::slice::from_ref(&token)).expect("valid");
  let id = save_in(repo.path(), None, draft).expect("saved");
  let connections = ProjectConnections::load(&db).expect("load");
  assert_eq!(connections.mcp.len(), 1);
  assert_eq!(connections.secrets.as_slice(), std::slice::from_ref(&token));
  assert!(connections.mcp_server(id).is_some_and(McpServer::enabled));
  set_enabled_in(repo.path(), id, false).expect("turned off");
  assert!(!db.mcp_server(id).expect("read").expect("stored").enabled());
  let renamed = McpServerText {
    name: "docs-2".to_owned(),
    ..stdio_text()
  }
  .draft(&[], std::slice::from_ref(&token))
  .expect("valid");
  assert_eq!(save_in(repo.path(), Some(id), renamed), Ok(id));
  let stored = db.mcp_server(id).expect("read").expect("stored");
  assert_eq!(stored.name().as_str(), "docs-2");
  assert!(!stored.enabled(), "editing keeps the server off");
}

/// One tool name per line, kept in order through saving; a name either CLI would rename, or one listed
/// twice, says why it cannot be saved.
#[test]
fn mcp_dialog_keeps_the_tools_that_run_without_asking() {
  let (repo, db) = project_db();
  let text = McpServerText {
    exempt_tools: "get_screenshot

batch_get
"
    .to_owned(),
    variables: String::new(),
    ..stdio_text()
  };
  let id = save_in(repo.path(), None, text.draft(&[], &[]).expect("valid")).expect("saved");
  let stored = db.mcp_server(id).expect("read").expect("stored");
  assert_eq!(
    McpServerText::of(&stored, &[]).exempt_tools,
    "get_screenshot
batch_get"
  );
  for (tools, key) in [
    ("get screenshot", "connections.mcp.errors.exemptToolName"),
    (
      "batch_get
batch_get",
      "connections.mcp.errors.exemptToolTwice",
    ),
    (&"a".repeat(65)[..], "connections.mcp.errors.exemptToolTooLong"),
  ] {
    let errors = McpServerText {
      exempt_tools: tools.to_owned(),
      ..text.clone()
    }
    .draft(&[], &[])
    .expect_err("tools it cannot save");
    assert_eq!(errors.exempt_tools.map(|message| message.key), Some(key), "{tools}");
  }
  let cleared = McpServerText {
    exempt_tools: String::new(),
    ..text
  };
  save_in(repo.path(), Some(id), cleared.draft(&[], &[]).expect("valid")).expect("saved");
  assert!(
    db.mcp_server(id)
      .expect("read")
      .expect("stored")
      .exempt_tools()
      .is_empty()
  );
}

fn remote(name: &str, exempt: &str, enabled: bool) -> McpServer {
  let mut server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(name).expect("valid"),
    McpTransport::Http(HttpServer::new(
      McpUrl::new("http://localhost:7/mcp").expect("valid"),
      None,
    )),
    McpEnvironment::default(),
    Timestamp::from_unix_millis(1),
  )
  .expect("a valid server");
  server.set_exempt_tools(ExemptTools::parse(exempt).expect("valid tools"));
  server.set_enabled(enabled);
  server
}

/// A step's permissions name the servers of its profile that run tools without asking: chosen, on,
/// and listing some.
#[test]
fn mcp_pipeline_steps_see_which_servers_run_tools_without_asking() {
  let (_repo, mut db) = project_db();
  let servers = [
    remote(
      "pencil",
      "get_screenshot
batch_get",
      true,
    ),
    remote("docs", "read", false),
    remote("notes", "", true),
    remote("unchosen", "read", true),
  ];
  for server in &servers {
    db.create_mcp_server(server).expect("server");
  }
  let chosen = servers[..3].iter().map(McpServer::id).collect();
  let body = AgentProfile::new(
    Name::new("Builder").expect("valid"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("haiku").expect("model"),
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(chosen).expect("distinct servers"));
  let profile = db
    .create_resource(
      Slug::new("builder").expect("slug"),
      Name::new("Builder").expect("valid"),
      &body,
      Timestamp::from_unix_millis(1),
    )
    .expect("profile");
  db.publish_draft::<AgentProfile>(profile.id(), Timestamp::from_unix_millis(2))
    .expect("published");
  let assignables = Assignables::load(&db).expect("load");
  let choice = assignables.profile(profile.id()).expect("the published profile");
  assert_eq!(assignables.exempt_tools(choice), [("pencil".into(), 2)]);
}
