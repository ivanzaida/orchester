//! What a change of `add_mcp_server`, `update_mcp_server`, `set_mcp_server_enabled` or `remove_mcp_server` would leave,
//! read without storing anything, for the Orchestrator chat's cards (ORC-89): the server as Add / Edit MCP server
//! would save it from the same fields and rules, what it widens by the same reasons the tools judge, and a digest of
//! the server as stored now, so that applying it later is refused once anything changed. A stdio program is looked up
//! as saving it would (`tools::program`), so the card names the file it would be; one not found is shown so, and
//! applying it is refused. A secret is named, never its value; a plain variable's value and the values of an
//! endpoint's query are not shown.

use std::path::Path;

use orchester_core::{
  connection::mcp::{
    ExemptTools, McpAuthorization, McpEnvValue, McpEnvironment, McpServer, McpServerName, McpTransport,
  },
  id::SecretId,
  orchestrator::mcp::{
    McpAuthorizationShown, McpChange, McpOperation, McpServerShown, McpTransportShown, McpVariableShown,
  },
  secret::Secret,
  store::SecretStore,
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::mcp_servers::{
  AddMcpServer, InServer, SetMcpServerEnabled, UpdateMcpServer, checked, filled, save_refusal, server_named,
  without_query_values,
};
use crate::{
  app_mcp::{error::ToolError, services::Services},
  connections::mcp::{McpSaveError, McpServerText, enabling, profiles_using, widenings},
  messages::Message,
  tools::program::{ProgramProblem, fixed_program},
};

/// A change as a card proposes it, and what it widens in the tools' words.
pub(crate) struct McpPreview {
  pub change: McpChange,
  pub widens: Vec<Message>,
}

/// The change `operation` with the tool's `arguments` (their `project` the chat's own) would make, refused as the tool
/// refuses it: fields the page refuses, a name another server has, a secret the project has not, a program that cannot
/// be stored but for one not found, a server profiles still choose for a removal. Blocks on SQLite and the disk.
pub(crate) fn preview(
  services: &Services,
  operation: McpOperation,
  arguments: &JsonObject,
) -> Result<McpPreview, ToolError> {
  let project = arguments.get("project").and_then(Value::as_str).unwrap_or_default();
  let open = services.project(project)?;
  let db = ProjectDb::open(&open.repo_root)?;
  let secrets = db.secrets()?;
  let mut arguments = arguments.clone();
  let read = Read {
    db: &db,
    secrets: &secrets,
    root: &open.repo_root,
  };
  let (stored, server, widens) = match operation {
    McpOperation::Add => {
      let asked: AddMcpServer = parse(&arguments)?;
      let draft = checked(
        &filled(McpServerText::new_server(), asked.fields, &secrets)?,
        &db,
        None,
        &secrets,
      )?;
      let widens = widenings(None, &draft);
      let shown = read.drafted(
        (&draft.name, &draft.transport),
        (&draft.environment, &draft.exempt_tools),
        None,
      )?;
      (None, shown, widens)
    }
    McpOperation::Update => {
      let asked: UpdateMcpServer = parse(&arguments)?;
      let stored = server_named(&db, &asked.server)?;
      let text = filled(McpServerText::of(&stored, &secrets), asked.fields, &secrets)?;
      let draft = checked(&text, &db, Some(&stored), &secrets)?;
      let widens = widenings(Some(&stored), &draft);
      let shown = read.drafted(
        (&draft.name, &draft.transport),
        (&draft.environment, &draft.exempt_tools),
        Some(&stored),
      )?;
      (Some(stored), shown, widens)
    }
    McpOperation::Enable | McpOperation::Disable => {
      let asked: SetMcpServerEnabled = parse(&arguments)?;
      let stored = server_named(&db, &asked.server)?;
      let mut shown = read.stored(&stored)?;
      shown.enabled = asked.enabled;
      (Some(stored.clone()), shown, enabling(&stored, asked.enabled))
    }
    McpOperation::Remove => {
      let asked: InServer = parse(&arguments)?;
      let stored = server_named(&db, &asked.server)?;
      let shown = read.stored(&stored)?;
      if !shown.used_by.is_empty() {
        let roles = shown.used_by.iter().map(|role| role.as_str().into()).collect();
        return Err(save_refusal(&McpSaveError::InUse(roles)));
      }
      (Some(stored), shown, Vec::new())
    }
  };
  // Applied later, the change names the server it was read for by its id: a name could name another one by then.
  if let Some(stored) = &stored {
    arguments.insert("server".to_owned(), Value::String(stored.id().to_string()));
  }
  Ok(McpPreview {
    change: McpChange {
      operation,
      id: stored.as_ref().map(McpServer::id),
      server,
      widens: !widens.is_empty(),
      basis: stored.as_ref().map(basis).unwrap_or_default(),
      arguments,
    },
    widens,
  })
}

/// What a preview reads with: the project's database, its active secrets and its repository's folder.
struct Read<'a> {
  db: &'a ProjectDb,
  secrets: &'a [Secret],
  root: &'a Path,
}

impl Read<'_> {
  /// `server` as it is stored.
  fn stored(&self, server: &McpServer) -> Result<McpServerShown, ToolError> {
    let mut shown = self.shown(
      (server.name(), server.transport()),
      (server.environment(), server.exempt_tools()),
    );
    shown.enabled = server.enabled();
    shown.used_by = self.used_by(server)?;
    Ok(shown)
  }

  /// The server a save would store from these parts, over `stored` (`None`: a new server, on once added). A program
  /// that cannot be stored is refused here as saving it would be, but for one not found, which the card warns of.
  fn drafted(
    &self,
    (name, transport): (&McpServerName, &McpTransport),
    (environment, exempt): (&McpEnvironment, &ExemptTools),
    stored: Option<&McpServer>,
  ) -> Result<McpServerShown, ToolError> {
    let mut shown = self.shown((name, transport), (environment, exempt));
    if let McpTransport::Stdio(stdio) = transport {
      match program_found(stdio.program(), self.root) {
        Err(ProgramProblem::NotFound(_)) | Ok(_) => {}
        Err(problem) => return Err(save_refusal(&McpSaveError::Program(problem))),
      }
    }
    if let Some(stored) = stored {
      shown.enabled = stored.enabled();
      shown.used_by = self.used_by(stored)?;
    }
    Ok(shown)
  }

  fn shown(
    &self,
    (name, transport): (&McpServerName, &McpTransport),
    (environment, exempt): (&McpEnvironment, &ExemptTools),
  ) -> McpServerShown {
    let transport = match transport {
      McpTransport::Stdio(stdio) => McpTransportShown::Stdio {
        program: stdio.program().as_str().to_owned(),
        found: program_found(stdio.program(), self.root)
          .ok()
          .map(|found| found.as_str().to_owned()),
        args: stdio.args().iter().map(|arg| arg.as_str().to_owned()).collect(),
        working_directory: stdio.working_directory().map(|folder| folder.as_str().to_owned()),
      },
      McpTransport::Http(http) => McpTransportShown::Http {
        url: without_query_values(http.url().as_str()),
        authorization: self.authorization(http.authorization()),
      },
    };
    McpServerShown {
      name: name.as_str().to_owned(),
      transport,
      environment: environment
        .iter()
        .map(|(name, value)| McpVariableShown {
          name: name.to_string(),
          secret: match value {
            McpEnvValue::Text(_) => None,
            McpEnvValue::Secret(id) => Some(self.secret(*id)),
          },
        })
        .collect(),
      exempt_tools: exempt.tools().iter().map(|tool| tool.as_str().to_owned()).collect(),
      enabled: true,
      used_by: Vec::new(),
    }
  }

  fn authorization(&self, authorization: &McpAuthorization) -> McpAuthorizationShown {
    match authorization {
      McpAuthorization::None => McpAuthorizationShown::None,
      McpAuthorization::Bearer { token } => McpAuthorizationShown::Bearer {
        secret: self.secret(*token),
      },
      McpAuthorization::Basic { username, password } => McpAuthorizationShown::Basic {
        username: username.as_str().to_owned(),
        secret: self.secret(*password),
      },
      McpAuthorization::BasicCredential { credential } => McpAuthorizationShown::BasicCredential {
        secret: self.secret(*credential),
      },
      McpAuthorization::Header { name, value } => McpAuthorizationShown::Header {
        name: name.as_str().to_owned(),
        secret: self.secret(*value),
      },
    }
  }

  /// Secret `id` by its name, `NAME@environment`; by its id while the project no longer lists it.
  fn secret(&self, id: SecretId) -> String {
    self.secrets.iter().find(|secret| secret.id() == id).map_or_else(
      || id.to_string(),
      |secret| format!("{}@{}", secret.name(), secret.environment().as_str()),
    )
  }

  fn used_by(&self, server: &McpServer) -> Result<Vec<String>, ToolError> {
    Ok(
      profiles_using(self.db, server.id())?
        .iter()
        .map(|role| role.to_string())
        .collect(),
    )
  }
}

/// The file `program` would be stored as, looked up on this app's search path as saving it looks it up.
fn program_found(
  program: &orchester_core::resource::CommandProgram,
  root: &Path,
) -> Result<orchester_core::resource::CommandProgram, ProgramProblem> {
  let path = orchester_exec::search_path::search_path();
  fixed_program(program, path.as_deref(), root)
}

/// A digest of `server` as it is stored, every field and whether it is on: a plain variable's value is in it, never in
/// what is kept.
fn basis(server: &McpServer) -> String {
  Sha256::digest(format!("{server:?}").as_bytes())
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect()
}

fn parse<T: DeserializeOwned>(arguments: &JsonObject) -> Result<T, ToolError> {
  serde_json::from_value(Value::Object(arguments.clone()))
    .map_err(|error| ToolError::InvalidArguments(error.to_string()))
}
