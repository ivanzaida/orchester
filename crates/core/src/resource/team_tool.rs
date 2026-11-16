//! The team's tool: what the team server holds of one (`docs/design/team-configuration.md`, "Who owns what": a tool is
//! the team's except its secret values). It names the secret an HTTP request sends as its bearer token, or a database
//! query connects with, by the secret's name, never by an id of somebody's computer: each computer fills in the value
//! under that name. Its JSON is the local [`Tool`]'s with a name where the local one holds an id, so a command tool,
//! which holds no secret, reads the same in both forms; the wire refuses any other field, and an id is no secret name,
//! so a body written for one computer is not a team tool.
//!
//! [`TeamTool::from_local`] is the one way a computer turns its tool into the team's: the publisher's secret id becomes
//! that secret's name. A local tool keeps its local form.

mod wire;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
  AccessPolicy, CommandAction, FieldName, HttpAuth, HttpError, HttpMethod, HttpRequest, RequestTarget, ResourceBody,
  ResourceKind, ResponseSource, Tool, ToolAction, ToolError, ToolKind,
};
use crate::{id::SecretId, secret::SecretName};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TeamToolError {
  #[error(transparent)]
  Tool(#[from] ToolError),
  #[error(transparent)]
  Http(#[from] HttpError),
  /// The local tool uses a secret this computer's project does not have, so the team cannot be told its name.
  #[error("the tool uses a secret this project does not have, so its name cannot be given to the team")]
  SecretNotHere,
}

/// A single HTTP request with a fixed URL, sending the value of the team secret `bearer` as its bearer token, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamHttpRequest {
  /// The request with no authentication of its own: its checks are the local request's.
  request: HttpRequest,
  bearer: Option<SecretName>,
}

impl TeamHttpRequest {
  pub fn new(
    method: HttpMethod,
    (url, allowed_host): (String, String),
    bearer: Option<SecretName>,
    inputs: BTreeMap<FieldName, RequestTarget>,
    outputs: BTreeMap<FieldName, ResponseSource>,
  ) -> Result<Self, HttpError> {
    let request = HttpRequest::new(method, url, allowed_host, HttpAuth::None, inputs, outputs)?;
    if bearer.is_some() && !request.is_https() {
      return Err(HttpError::InsecureBearer);
    }
    Ok(Self { request, bearer })
  }

  pub const fn method(&self) -> HttpMethod {
    self.request.method()
  }

  pub fn url(&self) -> &str {
    self.request.url()
  }

  pub fn allowed_host(&self) -> &str {
    self.request.allowed_host()
  }

  /// The team secret sent as the bearer token.
  pub const fn bearer(&self) -> Option<&SecretName> {
    self.bearer.as_ref()
  }

  pub fn inputs(&self) -> &BTreeMap<FieldName, RequestTarget> {
    self.request.inputs()
  }

  pub fn outputs(&self) -> &BTreeMap<FieldName, ResponseSource> {
    self.request.outputs()
  }

  /// The request without its authentication, which compares as a local one does.
  pub(crate) const fn unauthenticated(&self) -> &HttpRequest {
    &self.request
  }
}

/// A fixed query run with the credentials held in the team secret `credentials`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamDatabaseQuery {
  credentials: SecretName,
  query: String,
}

impl TeamDatabaseQuery {
  pub fn new(credentials: SecretName, query: String) -> Result<Self, ToolError> {
    if query.trim().is_empty() {
      return Err(ToolError::BlankQuery);
    }
    Ok(Self { credentials, query })
  }

  pub const fn credentials(&self) -> &SecretName {
    &self.credentials
  }

  pub fn query(&self) -> &str {
    &self.query
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamToolAction {
  Http(TeamHttpRequest),
  Command(CommandAction),
  DatabaseQuery(TeamDatabaseQuery),
}

impl TeamToolAction {
  pub const fn kind(&self) -> ToolKind {
    match self {
      Self::Http(_) => ToolKind::Http,
      Self::Command(_) => ToolKind::Command,
      Self::DatabaseQuery(_) => ToolKind::Database,
    }
  }
}

/// A tool of a team project, as a versioned resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "wire::TeamToolWire", into = "wire::TeamToolWire")]
pub struct TeamTool {
  description: String,
  action: TeamToolAction,
  access: Option<AccessPolicy>,
}

impl TeamTool {
  /// A tool that runs `action`. HTTP and database tools need an access policy, as a local one does.
  pub fn new(description: &str, action: TeamToolAction, access: Option<AccessPolicy>) -> Result<Self, ToolError> {
    if access.is_none() && action.kind() != ToolKind::Command {
      return Err(ToolError::AccessPolicyRequired);
    }
    Ok(Self {
      description: description.trim().to_owned(),
      action,
      access,
    })
  }

  /// `local` in the team's form: the secret it sends or connects with, held by id on this computer, becomes the name
  /// `secret_name` gives that id. Fails with [`TeamToolError::SecretNotHere`] for an id it gives no name.
  pub fn from_local(
    local: &Tool,
    secret_name: impl FnOnce(SecretId) -> Option<SecretName>,
  ) -> Result<Self, TeamToolError> {
    let action = match local.action() {
      ToolAction::Command(command) => TeamToolAction::Command(command.clone()),
      ToolAction::Http(request) => {
        let bearer = match request.auth() {
          HttpAuth::None => None,
          HttpAuth::Bearer { secret } => Some(secret_name(secret).ok_or(TeamToolError::SecretNotHere)?),
        };
        TeamToolAction::Http(TeamHttpRequest::new(
          request.method(),
          (request.url().to_owned(), request.allowed_host().to_owned()),
          bearer,
          request.inputs().clone(),
          request.outputs().clone(),
        )?)
      }
      ToolAction::DatabaseQuery(query) => {
        let credentials = secret_name(query.credentials()).ok_or(TeamToolError::SecretNotHere)?;
        TeamToolAction::DatabaseQuery(TeamDatabaseQuery::new(credentials, query.query().to_owned())?)
      }
    };
    Ok(Self::new(local.description(), action, local.access().cloned())?)
  }

  pub fn description(&self) -> &str {
    &self.description
  }

  pub const fn action(&self) -> &TeamToolAction {
    &self.action
  }

  pub const fn kind(&self) -> ToolKind {
    self.action.kind()
  }

  pub const fn access(&self) -> Option<&AccessPolicy> {
    self.access.as_ref()
  }

  /// The command it runs; `None` for an HTTP or database tool.
  pub const fn command(&self) -> Option<&CommandAction> {
    match &self.action {
      TeamToolAction::Command(command) => Some(command),
      TeamToolAction::Http(_) | TeamToolAction::DatabaseQuery(_) => None,
    }
  }

  /// A command tool as this computer runs it, which is the same on every computer: a command holds no secret. `None`
  /// for an HTTP or database tool.
  pub fn command_tool(&self) -> Option<Tool> {
    let command = self.command()?.clone();
    Some(
      Tool::new(&self.description, ToolAction::Command(command), self.access.clone())
        .expect("a command tool is a tool with or without an access policy"),
    )
  }

  /// The team secret it sends or connects with; a command tool holds none.
  pub const fn secret(&self) -> Option<&SecretName> {
    match &self.action {
      TeamToolAction::Http(request) => request.bearer(),
      TeamToolAction::DatabaseQuery(query) => Some(query.credentials()),
      TeamToolAction::Command(_) => None,
    }
  }
}

/// The local tool's kind: the team server keeps both forms under it, an old version in the local one.
impl ResourceBody for TeamTool {
  const KIND: ResourceKind = ResourceKind::Tool;
  const SCHEMA: u32 = 1;
}
