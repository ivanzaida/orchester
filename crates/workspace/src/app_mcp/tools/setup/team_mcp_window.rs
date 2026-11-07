//! The window's new team MCP server (`Agents · New MCP server` A8 in `acFkz`): what its form holds, made into the
//! team's body by the same rules `add_mcp_server` applies ([`team_mcp_body::new_server`]), and created as v1 with no
//! draft. The version is the team's only once the team server takes it ([`crate::sharing::Servers::send_created`]);
//! this creates it as the version that waits for the server. A person's decision to add a server is the window's to
//! ask: an automation's input is refused by the caller unless a person lets automation change agent permissions.

use orchester_core::{connection::mcp::McpServerName, id::ResourceId, resource::TeamMcpServer, time::Timestamp};
use orchester_store::{ProjectDb, StoreError};

use super::{
  mcp_server_input::{SecretHeader, ServerFields},
  team_mcp_body::new_server,
  team_mcp_servers::{create_published_server, require_admin, team_names},
};
use crate::{app_mcp::error::ToolError, messages::Message};

/// How the new server runs, as the form has it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Runs {
  /// A program each computer starts: its name, the arguments, the folder (relative to the run's worktree; empty is the
  /// worktree) and the names of the variables it needs, each filled from the team secret of its own name.
  Program {
    program: String,
    arguments: Vec<String>,
    folder: String,
    variables: Vec<String>,
  },
  /// A server reached over HTTP, with at most one header that carries a secret: the CLI is given one per server.
  Remote { url: String, header: Option<Header> },
}

/// A header a remote server is sent a secret in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
  pub name: String,
  /// The team secret its value comes from, by name.
  pub secret: String,
  /// Written before the value (`Bearer `); empty for none.
  pub prefix: String,
}

/// The form's fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTeamMcpServer {
  pub name: String,
  pub runs: Runs,
  /// The tools that run without asking, as the server names them.
  pub exempt_tools: Vec<String>,
}

/// Why the fields do not make a team MCP server: the field it is about, or the form's as a whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalid {
  Name(Message),
  Program(Message),
  Folder(Message),
  Url(Message),
  Form(Vec<Message>),
}

/// The names of the team's MCP servers, which a new one may not take.
pub fn team_mcp_names(root: &std::path::Path) -> Result<Vec<McpServerName>, StoreError> {
  team_names(&ProjectDb::open(root)?)
}

/// The team server `input` describes, its name not one of `taken`, as the tools make one.
pub fn team_mcp_server(input: &NewTeamMcpServer, taken: &[McpServerName]) -> Result<TeamMcpServer, Box<Invalid>> {
  let mut fields = ServerFields {
    name: Some(input.name.clone()),
    exempt_tools: Some(input.exempt_tools.clone()),
    ..ServerFields::default()
  };
  match &input.runs {
    Runs::Program {
      program,
      arguments,
      folder,
      variables,
    } => {
      fields.program = Some(program.clone());
      fields.args = Some(arguments.clone());
      fields.working_directory = Some(folder.clone());
      fields.variable_names = Some(variables.clone());
    }
    Runs::Remote { url, header } => {
      fields.url = Some(url.clone());
      fields.secret_headers = Some(
        header
          .iter()
          .map(|header| SecretHeader {
            header: header.name.clone(),
            secret: header.secret.clone(),
            prefix: Some(header.prefix.clone()).filter(|prefix| !prefix.is_empty()),
          })
          .collect(),
      );
    }
  }
  new_server(fields, taken).map_err(|error| Box::new(invalid(error)))
}

impl Invalid {
  /// The first reason it gives, for a caller that has one place to say it.
  pub fn message(self) -> Message {
    match self {
      Self::Name(message) | Self::Program(message) | Self::Folder(message) | Self::Url(message) => message,
      Self::Form(mut messages) => {
        if messages.is_empty() {
          Message::new("appMcp.errors.teamMcpInvalid")
        } else {
          messages.remove(0)
        }
      }
    }
  }
}

fn invalid(error: ToolError) -> Invalid {
  let ToolError::Refused { code, mut reasons } = error else {
    return Invalid::Form(vec![Message::new("errors.storage")]);
  };
  if reasons.is_empty() {
    return Invalid::Form(reasons);
  }
  let first = reasons.remove(0);
  match code.as_str() {
    "name_taken" => Invalid::Name(first),
    "team_path_refused" if first.key.contains("Folder") => Invalid::Folder(first),
    "team_path_refused" => Invalid::Program(first),
    "team_value_refused" => Invalid::Url(first),
    _ => {
      reasons.insert(0, first);
      Invalid::Form(reasons)
    }
  }
}

/// Creates `server` as v1 of a new team MCP server, waiting for the team server; refused unless the person may change
/// the team's configuration.
pub fn create_team_mcp_server(root: &std::path::Path, server: &TeamMcpServer) -> Result<ResourceId, StoreError> {
  let mut db = ProjectDb::open(root)?;
  require_admin(&db).map_err(|_| StoreError::NotAdmin)?;
  create_published_server(&mut db, server, Timestamp::now())
}
