//! MCP servers as the Connections screen edits them (`orchester_core::connection::mcp`): the
//! dialog's fields as entered and their mapping to and from a server, and saving a server or
//! turning it on and off. Arguments are one per line and plain variables `NAME=value` lines, as a
//! command tool's; a variable whose value comes from a project secret is written
//! `NAME=${secret:SECRET_NAME@environment}`, which names the secret, never its value. The tools that
//! run without asking are one name per line. Saving blocks on SQLite; the UI calls it off the UI thread.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  connection::mcp::{
    ExemptTools, ExemptToolsError, HttpServer, McpEnvValue, McpEnvironment, McpServer, McpServerError, McpServerName,
    McpServerNameError, McpToolName, McpToolNameError, McpTransport, McpUrl, StdioServer, TransportError,
    WorkingDirectory,
  },
  id::{ConnectionId, SecretId},
  resource::{CommandArg, CommandProgram, VariableName},
  secret::Secret,
  store::McpServerStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  messages::{self, Message},
  tools::form::{parse_arguments, program_message},
};

const SECRET_OPEN: &str = "${secret:";
const SECRET_CLOSE: &str = "}";

/// How the server is reached, as the dialog's Transport select offers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportChoice {
  Stdio,
  Http,
}

impl lurq::app::component::DevtoolsInspectable for TransportChoice {}

/// The dialog's fields as the user left them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpServerText {
  pub name: String,
  pub transport: TransportChoice,
  pub program: String,
  /// One argument per line; empty lines are left out.
  pub arguments: String,
  /// Empty: the worktree.
  pub working_directory: String,
  /// `NAME=value` per line, a secret written `${secret:NAME@environment}`.
  pub variables: String,
  pub url: String,
  pub bearer: Option<SecretId>,
  /// One tool name per line; empty lines are left out.
  pub exempt_tools: String,
}

/// Why the server cannot be saved yet, per field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpServerErrors {
  pub name: Option<Message>,
  pub program: Option<Message>,
  pub arguments: Option<Message>,
  pub working_directory: Option<Message>,
  pub variables: Option<Message>,
  pub url: Option<Message>,
  pub exempt_tools: Option<Message>,
}

/// A server as the dialog saves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpServerDraft {
  pub name: McpServerName,
  pub transport: McpTransport,
  pub environment: McpEnvironment,
  pub exempt_tools: ExemptTools,
}

impl McpServerText {
  /// A new server: a program the CLI starts, nothing entered yet.
  pub fn new_server() -> Self {
    Self {
      name: String::new(),
      transport: TransportChoice::Stdio,
      program: String::new(),
      arguments: String::new(),
      working_directory: String::new(),
      variables: String::new(),
      url: String::new(),
      bearer: None,
      exempt_tools: String::new(),
    }
  }

  /// The fields of `server`, secrets named as `secrets` has them.
  pub fn of(server: &McpServer, secrets: &[Secret]) -> Self {
    let mut text = Self {
      name: server.name().as_str().to_owned(),
      variables: variables_text(server.environment(), secrets),
      exempt_tools: server
        .exempt_tools()
        .tools()
        .iter()
        .map(McpToolName::as_str)
        .collect::<Vec<_>>()
        .join(
          "
",
        ),
      ..Self::new_server()
    };
    match server.transport() {
      McpTransport::Stdio(stdio) => {
        text.program = stdio.program().as_str().to_owned();
        text.arguments = stdio
          .args()
          .iter()
          .map(CommandArg::as_str)
          .collect::<Vec<_>>()
          .join("\n");
        text.working_directory = stdio
          .working_directory()
          .map_or_else(String::new, |directory| directory.as_str().to_owned());
      }
      McpTransport::Http(http) => {
        text.transport = TransportChoice::Http;
        text.url = http.url().as_str().to_owned();
        text.bearer = http.bearer();
      }
    }
    text
  }

  /// The server these fields describe, its name not one of `taken`, its secrets among `secrets`.
  pub fn draft(&self, taken: &[McpServerName], secrets: &[Secret]) -> Result<McpServerDraft, Box<McpServerErrors>> {
    let name = McpServerName::new(&self.name)
      .map_err(|error| name_message(&error))
      .and_then(|name| {
        if taken.contains(&name) {
          Err(Message::new("errors.connectionNameTaken"))
        } else {
          Ok(name)
        }
      });
    let mut errors = McpServerErrors {
      name: name.as_ref().err().cloned(),
      ..McpServerErrors::default()
    };
    let transport = match self.transport {
      TransportChoice::Stdio => self.stdio(&mut errors),
      TransportChoice::Http => self.http(&mut errors),
    };
    let environment = match self.transport {
      TransportChoice::Stdio => {
        parse_variables(&self.variables, secrets).map_err(|error| errors.variables = Some(error))
      }
      TransportChoice::Http => Ok(McpEnvironment::default()),
    };
    let exempt_tools =
      ExemptTools::parse(&self.exempt_tools).map_err(|error| errors.exempt_tools = Some(exempt_message(&error)));
    match (name, transport, environment, exempt_tools) {
      (Ok(name), Some(transport), Ok(environment), Ok(exempt_tools)) => Ok(McpServerDraft {
        name,
        transport,
        environment,
        exempt_tools,
      }),
      _ => Err(Box::new(errors)),
    }
  }

  fn stdio(&self, errors: &mut McpServerErrors) -> Option<McpTransport> {
    let program = CommandProgram::new(&self.program).map_err(|error| errors.program = Some(program_message(&error)));
    let arguments = parse_arguments(&self.arguments).map_err(|error| errors.arguments = Some(error));
    let directory = match self.working_directory.trim() {
      "" => Ok(None),
      directory => WorkingDirectory::new(directory)
        .map(Some)
        .map_err(|error| errors.working_directory = Some(transport_message(&error))),
    };
    let (Ok(program), Ok(arguments), Ok(directory)) = (program, arguments, directory) else {
      return None;
    };
    Some(McpTransport::Stdio(StdioServer::new(program, arguments, directory)))
  }

  fn http(&self, errors: &mut McpServerErrors) -> Option<McpTransport> {
    match McpUrl::new(&self.url) {
      Ok(url) => Some(McpTransport::Http(HttpServer::new(url, self.bearer))),
      Err(error) => {
        errors.url = Some(transport_message(&error));
        None
      }
    }
  }
}

/// `${secret:NAME@environment}`, how a variable names `secret`.
pub fn secret_reference(secret: &Secret) -> String {
  format!(
    "{SECRET_OPEN}{}@{}{SECRET_CLOSE}",
    secret.name(),
    secret.environment().as_str()
  )
}

fn variables_text(environment: &McpEnvironment, secrets: &[Secret]) -> String {
  environment
    .iter()
    .map(|(name, value)| {
      let value = match value {
        McpEnvValue::Text(text) => text.clone(),
        McpEnvValue::Secret(id) => secrets.iter().find(|secret| secret.id() == *id).map_or_else(
          // A secret the project no longer lists keeps its reference, so saving does not drop it.
          || format!("{SECRET_OPEN}{id}{SECRET_CLOSE}"),
          secret_reference,
        ),
      };
      format!("{name}={value}")
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// The variables of `NAME=value` lines; empty lines are left out.
pub fn parse_variables(text: &str, secrets: &[Secret]) -> Result<McpEnvironment, Message> {
  let mut variables = BTreeMap::new();
  for line in text.lines().filter(|line| !line.trim().is_empty()) {
    let (name, value) = line
      .split_once('=')
      .ok_or_else(|| Message::new("tools.errors.variableLine").with("line", line.to_owned()))?;
    let name = VariableName::new(name.trim())
      .map_err(|_| Message::new("tools.errors.variableName").with("name", name.trim().to_owned()))?;
    let value = match value
      .trim()
      .strip_prefix(SECRET_OPEN)
      .and_then(|rest| rest.strip_suffix(SECRET_CLOSE))
    {
      Some(reference) => McpEnvValue::Secret(secret_of(reference, secrets)?),
      None => McpEnvValue::Text(value.to_owned()),
    };
    if variables.insert(name.clone(), value).is_some() {
      return Err(Message::new("tools.errors.duplicateVariable").with("name", name.as_str().to_owned()));
    }
  }
  McpEnvironment::new(variables).map_err(|_| Message::new("connections.mcp.errors.variables"))
}

/// The secret `NAME@environment` (or a secret id kept from before) names.
fn secret_of(reference: &str, secrets: &[Secret]) -> Result<SecretId, Message> {
  if let Ok(id) = reference.parse::<SecretId>() {
    return Ok(id);
  }
  secrets
    .iter()
    .find(|secret| format!("{}@{}", secret.name(), secret.environment().as_str()) == reference)
    .map(Secret::id)
    .ok_or_else(|| Message::new("connections.mcp.errors.unknownSecret").with("secret", reference.to_owned()))
}

fn name_message(error: &McpServerNameError) -> Message {
  match error {
    McpServerNameError::Blank => Message::new("connections.mcp.errors.nameRequired"),
    McpServerNameError::TooLong => {
      Message::new("connections.mcp.errors.nameTooLong").with("max", McpServerName::MAX_CHARS.to_string())
    }
    McpServerNameError::Invalid => Message::new("connections.mcp.errors.nameInvalid"),
  }
}

fn exempt_message(error: &ExemptToolsError) -> Message {
  match error {
    ExemptToolsError::Name(McpToolNameError::Invalid(tool)) => {
      Message::new("connections.mcp.errors.exemptToolName").with("tool", tool.clone())
    }
    ExemptToolsError::Name(McpToolNameError::TooLong) => {
      Message::new("connections.mcp.errors.exemptToolTooLong").with("max", McpToolName::MAX_CHARS.to_string())
    }
    // Empty lines are left out, so a listed name is never blank.
    ExemptToolsError::Name(McpToolNameError::Blank) => {
      Message::new("connections.mcp.errors.exemptToolName").with("tool", String::new())
    }
    ExemptToolsError::Duplicate(tool) => {
      Message::new("connections.mcp.errors.exemptToolTwice").with("tool", tool.clone())
    }
    ExemptToolsError::TooMany => {
      Message::new("connections.mcp.errors.exemptToolsTooMany").with("max", ExemptTools::MAX.to_string())
    }
  }
}

fn transport_message(error: &TransportError) -> Message {
  match error {
    TransportError::BlankUrl => Message::new("connections.mcp.errors.urlRequired"),
    TransportError::UrlScheme => Message::new("connections.mcp.errors.urlScheme"),
    TransportError::UrlCredentials => Message::new("connections.mcp.errors.urlCredentials"),
    TransportError::Whitespace(_) => Message::new("connections.mcp.errors.whitespace"),
    TransportError::TooLong(_) => Message::new("connections.mcp.errors.tooLong"),
    TransportError::BlankWorkingDirectory | TransportError::OutsideWorktree(_) => {
      Message::new("connections.mcp.errors.workingDirectory")
    }
  }
}

/// Stores `draft` as a new server, or as server `id`'s new settings.
pub fn save_in(repo_root: &Path, id: Option<ConnectionId>, draft: McpServerDraft) -> Result<ConnectionId, Message> {
  let saved = save(
    &mut ProjectDb::open(repo_root).map_err(|error| store_message(&error))?,
    id,
    draft,
  );
  saved.map_err(|error| match error {
    SaveError::Store(error) => store_message(&error),
    SaveError::Server(McpServerError::EnvironmentOnHttp) => Message::new("connections.mcp.errors.variables"),
  })
}

enum SaveError {
  Store(StoreError),
  Server(McpServerError),
}

fn save(db: &mut ProjectDb, id: Option<ConnectionId>, draft: McpServerDraft) -> Result<ConnectionId, SaveError> {
  let Some(id) = id else {
    let mut server = McpServer::new(
      ConnectionId::generate(),
      draft.name,
      draft.transport,
      draft.environment,
      Timestamp::now(),
    )
    .map_err(SaveError::Server)?;
    server.set_exempt_tools(draft.exempt_tools);
    db.create_mcp_server(&server).map_err(SaveError::Store)?;
    return Ok(server.id());
  };
  let mut server = db
    .mcp_server(id)
    .map_err(SaveError::Store)?
    .ok_or(SaveError::Store(StoreError::ConnectionNotFound(id)))?;
  server
    .update(draft.name, draft.transport, draft.environment)
    .map_err(SaveError::Server)?;
  server.set_exempt_tools(draft.exempt_tools);
  db.save_mcp_server(&server).map_err(SaveError::Store)?;
  Ok(id)
}

/// Turns server `id` on or off for every profile that chooses it.
pub fn set_enabled_in(repo_root: &Path, id: ConnectionId, enabled: bool) -> Result<(), StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  let mut server = db.mcp_server(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  server.set_enabled(enabled);
  db.save_mcp_server(&server)
}

fn store_message(error: &StoreError) -> Message {
  tracing::error!(%error, "cannot save the MCP server");
  messages::store_error(error)
}
