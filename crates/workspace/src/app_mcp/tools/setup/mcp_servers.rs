//! `add_mcp_server`, `update_mcp_server`, `remove_mcp_server` and `set_mcp_server_enabled`: the project's MCP
//! servers as Connections › MCP servers adds, edits, turns on or off and removes them, through the same
//! fields and rules (`connections::mcp`). What widens what agents may do (`connections::mcp::widenings`: a new
//! server, a new name, another program, arguments, folder or endpoint, another authorization that sends a secret, a
//! variable but the harmless ones, a variable that takes a secret, a tool that runs without asking, turning a server
//! on) is refused as `person_only` unless a person lets automation change agent permissions, judged by the fields'
//! text before anything reads the disk; removing, turning off and dropping exempt tools are always taken. A program
//! is stored as the fixed file it is, outside the repository, as a command tool's (`tools::program`). A secret is
//! named, never its value, and only the project's active secrets can be named.

use std::path::Path;

use orchester_core::{
  connection::mcp::{McpAuthorization, McpEnvValue, McpServer, McpServerName, McpTransport},
  id::{ConnectionId, ProjectId, SecretId},
  secret::Secret,
  store::{McpServerStore, SecretStore},
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::widening;
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  connections::{
    data::ProjectConnections,
    mcp::{
      self, Allowed, AuthorizationChoice, AuthorizationText, McpSaveError, McpServerText, NAME_TAKEN, TransportChoice,
      enabling, widenings,
    },
  },
  links::Link,
  messages::Message,
  tasks::outside::Outside,
  tools::program::ProgramProblem,
};

/// A variable the server's program gets.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct Variable {
  pub name: String,
  /// The value as written; `${secret:NAME@environment}` takes the value of the project secret NAME of that
  /// environment when an agent starts.
  pub value: String,
}

/// What a remote server is sent with every request. Each `secret` is an active project secret named
/// `NAME@environment`, whose value is read when an agent starts; no tool takes a secret's value.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "scheme", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub enum Authorization {
  /// Nothing.
  None,
  /// `Authorization: Bearer <the secret's value>`.
  Bearer { secret: String },
  /// `Authorization: Basic <base64 of username:password>`, the password the secret's value.
  Basic { username: String, secret: String },
  /// `Authorization: Basic <credential>`, the secret's value a Basic credential pasted whole: its base64 part, or
  /// the header's whole value starting with `Basic `.
  BasicCredential { secret: String },
  /// `<name>: <the secret's value>`, for a server that takes another header or scheme (`X-API-Key`).
  Header { name: String, secret: String },
}

/// The fields of a server, as Add MCP server and Edit MCP server have them.
#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ServerFields {
  /// Small letters, digits, - and _, starting with a letter; agents know the server and its tools by it.
  pub name: Option<String>,
  /// A program on this computer, which the agent's CLI starts with each agent: an absolute path to an
  /// existing file. Not with `url`.
  pub program: Option<String>,
  /// The program's arguments, passed as written; none may be empty or hold a line break.
  pub args: Option<Vec<String>>,
  /// Where the program starts: a folder inside the worktree or an absolute path; empty: the worktree.
  pub working_directory: Option<String>,
  /// The program's variables.
  pub environment: Option<Vec<Variable>>,
  /// A remote server the agent's CLI reaches over HTTP: its http:// or https:// URL. Not with `program`.
  pub url: Option<String>,
  /// What a remote server is sent to let the agent in; scheme none sends nothing.
  pub authorization: Option<Authorization>,
  /// Tools of the server that run without asking, as the server names them, for profiles that ask
  /// before MCP actions.
  pub exempt_tools: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct AddMcpServer {
  /// The project's id, or its name.
  pub project: String,
  #[serde(flatten)]
  pub fields: ServerFields,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct UpdateMcpServer {
  /// The project's id, or its name.
  pub project: String,
  /// The server's id, or its name.
  pub server: String,
  /// The fields to change; one left out keeps its value.
  #[serde(flatten)]
  pub fields: ServerFields,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InServer {
  /// The project's id, or its name.
  pub project: String,
  /// The server's id, or its name.
  pub server: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetMcpServerEnabled {
  /// The project's id, or its name.
  pub project: String,
  /// The server's id, or its name.
  pub server: String,
  /// On: every profile that chooses it gives it to its agents from their next start; off: none does.
  pub enabled: bool,
}

pub async fn add_mcp_server(services: &Services, args: AddMcpServer) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let secrets = db.secrets()?;
    let text = filled(McpServerText::new_server(), args.fields, &secrets)?;
    let draft = checked(&text, &db, None, &secrets)?;
    let allowed = allowance(&services, widenings(None, &draft))?;
    let id = mcp::store_in(&open.repo_root, None, draft, &allowed).map_err(|error| save_refusal(&error))?;
    services.changes.record(open.project.id(), Outside::Connections);
    server_result(&open.repo_root, id)
  })
  .await
}

pub async fn update_mcp_server(services: &Services, args: UpdateMcpServer) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let stored = server_named(&db, &args.server)?;
    let secrets = db.secrets()?;
    let text = filled(McpServerText::of(&stored, &secrets), args.fields, &secrets)?;
    let draft = checked(&text, &db, Some(&stored), &secrets)?;
    let allowed = allowance(&services, widenings(Some(&stored), &draft))?;
    mcp::store_in(&open.repo_root, Some(stored.id()), draft, &allowed).map_err(|error| save_refusal(&error))?;
    services.changes.record(open.project.id(), Outside::Connections);
    server_result(&open.repo_root, stored.id())
  })
  .await
}

pub async fn remove_mcp_server(services: &Services, args: InServer) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let server = server_named(&ProjectDb::open(&open.repo_root)?, &args.server)?;
    mcp::remove_in(&open.repo_root, server.id()).map_err(|error| save_refusal(&error))?;
    services.changes.record(open.project.id(), Outside::Connections);
    Ok(json!({ "removed": { "id": server.id().to_string(), "name": server.name().as_str() } }))
  })
  .await
}

pub async fn set_mcp_server_enabled(services: &Services, args: SetMcpServerEnabled) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let server = server_named(&ProjectDb::open(&open.repo_root)?, &args.server)?;
    widening::allowed(&services, enabling(&server, args.enabled))?;
    mcp::set_enabled_in(&open.repo_root, server.id(), args.enabled)?;
    services.changes.record(open.project.id(), Outside::Connections);
    server_result(&open.repo_root, server.id())
  })
  .await
}

/// The MCP servers of `project`, as `list_connections` lists them, each with its link: its inspector in
/// Connections.
pub(super) fn servers_json(listed: &ProjectConnections, project: ProjectId) -> Vec<Value> {
  listed
    .mcp
    .iter()
    .map(|server| {
      let mut value = server_json(listed, server);
      value["link"] = json!(Link::Connection(project, server.id()).path());
      value
    })
    .collect()
}

/// `server` with the fields Edit MCP server shows, and the roles of the profiles that choose it. A secret is named
/// (`NAME@environment`), never its value; a variable's plain value and the values of the endpoint's query are left
/// out, since a person may have pasted a token there: automation sees that they are set, not what they hold.
fn server_json(listed: &ProjectConnections, server: &McpServer) -> Value {
  let secrets = &listed.secrets;
  let secret_named = |id| {
    secrets
      .iter()
      .find(|secret| secret.id() == id)
      .map(|secret| format!("{}@{}", secret.name(), secret.environment().as_str()))
  };
  let transport = match server.transport() {
    McpTransport::Stdio(stdio) => json!({
      "kind": "stdio",
      "program": stdio.program().as_str(),
      "args": stdio.args().iter().map(|arg| arg.as_str()).collect::<Vec<_>>(),
      "working_directory": stdio.working_directory().map(|folder| folder.as_str()),
    }),
    McpTransport::Http(http) => json!({
      "kind": "http",
      "url": without_query_values(http.url().as_str()),
      "authorization": authorization_json(http.authorization(), secret_named),
    }),
  };
  let environment: Vec<Value> = server
    .environment()
    .iter()
    .map(|(name, value)| match value {
      McpEnvValue::Text(_) => json!({ "name": name.to_string(), "kind": "text" }),
      McpEnvValue::Secret(id) => json!({
        "name": name.to_string(),
        "kind": "secret",
        "secret": secret_named(*id),
      }),
    })
    .collect();
  let used_by: Vec<&str> = listed.used_by(server.id()).map(|used| &*used.role).collect();
  json!({
    "id": server.id().to_string(),
    "name": server.name().as_str(),
    "enabled": server.enabled(),
    "transport": transport,
    "environment": environment,
    "exempt_tools": server.exempt_tools().tools().iter().map(|tool| tool.as_str()).collect::<Vec<_>>(),
    "used_by": used_by,
  })
}

/// `url` with each query value replaced by `…`, its names kept, and a fragment's text left out.
pub(super) fn without_query_values(url: &str) -> String {
  let Some((base, query)) = url.split_once('?') else {
    return url.to_owned();
  };
  let (query, fragment) = query.split_once('#').map_or((query, false), |(query, _)| (query, true));
  let names: Vec<String> = query
    .split('&')
    .map(|pair| match pair.split_once('=') {
      Some((name, _)) => format!("{name}=…"),
      None => pair.to_owned(),
    })
    .collect();
  let fragment = if fragment { "#…" } else { "" };
  format!("{base}?{}{fragment}", names.join("&"))
}

fn server_result(repo_root: &Path, id: ConnectionId) -> Result<Value, ToolError> {
  let listed = ProjectConnections::load_in(repo_root)?;
  let server = listed
    .mcp_server(id)
    .ok_or_else(|| ToolError::not_found(Missing::McpServer, id.to_string()))?;
  Ok(json!({ "server": server_json(&listed, server) }))
}

/// The project's server named by `named`: its id, or else its name (a name never reads as an id).
pub(super) fn server_named(db: &ProjectDb, named: &str) -> Result<McpServer, ToolError> {
  let servers = db.mcp_servers()?;
  let by_id = servers.iter().position(|server| server.id().to_string() == named);
  let by_name = || servers.iter().position(|server| server.name().as_str() == named);
  by_id
    .or_else(by_name)
    .map(|index| servers[index].clone())
    .ok_or_else(|| ToolError::not_found(Missing::McpServer, named))
}

/// `text` with the `fields` given put in, as a person would type them into the page.
pub(super) fn filled(
  mut text: McpServerText,
  fields: ServerFields,
  secrets: &[Secret],
) -> Result<McpServerText, ToolError> {
  applies(&text, &fields)?;
  match (&fields.program, &fields.url) {
    (Some(_), Some(_)) => return Err(refused("invalid_transport", "appMcp.errors.mcpProgramAndUrl")),
    (Some(_), None) => text.transport = TransportChoice::Stdio,
    (None, Some(_)) => text.transport = TransportChoice::Http,
    (None, None) => {}
  }
  if let Some(name) = fields.name {
    text.name = name;
  }
  if let Some(program) = fields.program {
    // Taken as written: nothing reads the disk before the gate (`widening::allowed`) let the change through.
    text.program = program;
  }
  if let Some(args) = fields.args {
    text.arguments = lines(&args, "appMcp.errors.mcpArgument")?;
  }
  if let Some(folder) = fields.working_directory {
    text.working_directory = folder;
  }
  if let Some(variables) = fields.environment {
    let lines: Vec<String> = variables
      .into_iter()
      .map(|variable| format!("{}={}", variable.name, variable.value))
      .collect();
    text.variables = self::lines(&lines, "appMcp.errors.mcpVariable")?;
  }
  if let Some(url) = fields.url {
    text.url = url;
  }
  if let Some(authorization) = fields.authorization {
    text.authorization = authorization_text(authorization, secrets)?;
  }
  if let Some(tools) = fields.exempt_tools {
    text.exempt_tools = lines(&tools, "appMcp.errors.mcpExemptTool")?;
  }
  Ok(text)
}

/// The server `text` describes, checked as the page checks it; its name not another server's.
pub(super) fn checked(
  text: &McpServerText,
  db: &ProjectDb,
  stored: Option<&McpServer>,
  secrets: &[Secret],
) -> Result<mcp::McpServerDraft, ToolError> {
  let taken: Vec<McpServerName> = db
    .mcp_servers()?
    .into_iter()
    .filter(|server| Some(server.id()) != stored.map(McpServer::id))
    .map(|server| server.name().clone())
    .collect();
  text.draft(&taken, secrets).map_err(|errors| {
    let code = match &errors.name {
      Some(name) if name.key == NAME_TAKEN => "name_taken",
      _ => "invalid_server",
    };
    let reasons: Vec<Message> = errors.in_order().into_iter().flatten().cloned().collect();
    ToolError::refused(code, reasons)
  })
}

/// `values` one per line, none empty and none holding a line break.
fn lines(values: &[String], invalid: &'static str) -> Result<String, ToolError> {
  if let Some(value) = values
    .iter()
    .find(|value| value.trim().is_empty() || value.contains(['\n', '\r']))
  {
    return Err(ToolError::refused(
      "invalid_server",
      vec![Message::new(invalid).with("value", value.clone())],
    ));
  }
  Ok(values.join("\n"))
}

/// The authorization fields `authorization` fills in, its secret named as `secrets` has it.
fn authorization_text(authorization: Authorization, secrets: &[Secret]) -> Result<AuthorizationText, ToolError> {
  let (choice, secret, username, header) = match authorization {
    Authorization::None => return Ok(AuthorizationText::default()),
    Authorization::Bearer { secret } => (AuthorizationChoice::Bearer, secret, String::new(), String::new()),
    Authorization::Basic { username, secret } => (AuthorizationChoice::Basic, secret, username, String::new()),
    Authorization::BasicCredential { secret } => (
      AuthorizationChoice::BasicCredential,
      secret,
      String::new(),
      String::new(),
    ),
    Authorization::Header { name, secret } => (AuthorizationChoice::Header, secret, String::new(), name),
  };
  Ok(AuthorizationText {
    choice,
    secret: Some(secret_named_by(&secret, secrets)?),
    username,
    header,
  })
}

/// The active secret `NAME@environment` names.
fn secret_named_by(named: &str, secrets: &[Secret]) -> Result<SecretId, ToolError> {
  secrets
    .iter()
    .find(|secret| format!("{}@{}", secret.name(), secret.environment().as_str()) == named.trim())
    .map(Secret::id)
    .ok_or_else(|| {
      ToolError::refused(
        "invalid_server",
        vec![Message::new("connections.mcp.errors.unknownSecret").with("secret", named.to_owned())],
      )
    })
}

/// `authorization` as `list_connections` shows it: its scheme, the user name or header name, and its secret named.
fn authorization_json(authorization: &McpAuthorization, secret_named: impl Fn(SecretId) -> Option<String>) -> Value {
  let secret = authorization.secret().and_then(secret_named);
  match authorization {
    McpAuthorization::None => json!({ "scheme": "none" }),
    McpAuthorization::Bearer { .. } => json!({ "scheme": "bearer", "secret": secret }),
    McpAuthorization::Basic { username, .. } => {
      json!({ "scheme": "basic", "username": username.as_str(), "secret": secret })
    }
    McpAuthorization::BasicCredential { .. } => json!({ "scheme": "basic_credential", "secret": secret }),
    McpAuthorization::Header { name, .. } => json!({ "scheme": "header", "name": name.as_str(), "secret": secret }),
  }
}

/// Refuses a field the server's transport (after `fields` chose it) does not have, rather than dropping it.
fn applies(text: &McpServerText, fields: &ServerFields) -> Result<(), ToolError> {
  let transport = match (&fields.program, &fields.url) {
    (Some(_), _) => TransportChoice::Stdio,
    (None, Some(_)) => TransportChoice::Http,
    (None, None) => text.transport,
  };
  let misplaced = match transport {
    TransportChoice::Stdio => vec![("authorization", fields.authorization.is_some())],
    TransportChoice::Http => vec![
      ("args", fields.args.is_some()),
      ("working_directory", fields.working_directory.is_some()),
      ("environment", fields.environment.is_some()),
    ],
  };
  match misplaced.into_iter().find(|(_, given)| *given) {
    Some((field, _)) => Err(ToolError::refused(
      "field_not_applicable",
      vec![Message::new("appMcp.errors.mcpFieldNotApplicable").with("field", field.to_owned())],
    )),
    None => Ok(()),
  }
}

/// What a save may widen once [`widening::allowed`] let it: anything while a person lets automation widen, else
/// nothing, also when the server changed meanwhile (`mcp::store_in` checks the server as stored then).
fn allowance(services: &Services, reasons: Vec<Message>) -> Result<Allowed, ToolError> {
  let widens = !reasons.is_empty();
  widening::allowed(services, reasons)?;
  Ok(if widens {
    Allowed::Anything
  } else {
    Allowed::These(Vec::new())
  })
}

pub(super) fn save_refusal(error: &McpSaveError) -> ToolError {
  let code = match error {
    McpSaveError::Program(problem) => match problem {
      ProgramProblem::InWorktree => "program_in_worktree",
      ProgramProblem::NotFound(_) => "program_not_found",
      ProgramProblem::NotLocal(_) => "program_not_local",
      ProgramProblem::InRepository(_) => "program_in_repository",
      ProgramProblem::NotText(_) => "program_path_not_text",
      ProgramProblem::Invalid(_) => "invalid_server",
    },
    McpSaveError::Store(orchester_store::StoreError::DuplicateConnectionName(_)) => "name_taken",
    McpSaveError::Store(orchester_store::StoreError::ConnectionNotFound(_)) => "not_found",
    McpSaveError::InUse(_) => "in_use",
    McpSaveError::Changed(_) => "changed_meanwhile",
    McpSaveError::Store(_) | McpSaveError::Server(_) => "project_data",
  };
  let mut reasons = vec![error.message()];
  if let McpSaveError::Changed(widens) = error {
    reasons.extend(widens.iter().cloned());
  }
  ToolError::refused(code, reasons)
}

fn refused(code: &str, key: &'static str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key)])
}
