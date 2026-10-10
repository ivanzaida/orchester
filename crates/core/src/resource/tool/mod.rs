mod access;
mod command;
mod http;
mod launch;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  access::{AccessPolicy, AccessPolicyError, AccessWindow, AllowedSteps, ApprovalMode, DeniedBehavior},
  command::{
    CommandAction, CommandArg, CommandEnvironment, CommandError, CommandProgram, CommandTimeout, ExitCodes,
    OutputLimit, ProgramLocation, VariableName,
  },
  http::{FieldName, FieldNameError, HttpAuth, HttpError, HttpMethod, HttpRequest, RequestTarget, ResponseSource},
  launch::NO_CURRENT_DIRECTORY_IN_EXE_PATH,
};
use crate::id::SecretId;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolError {
  #[error("a database tool needs a query")]
  BlankQuery,
  /// HTTP and database tools release credentials, so who may call them must be said.
  #[error("an HTTP or database tool needs an access policy")]
  AccessPolicyRequired,
}

/// A fixed query run with the database credentials held in a project secret. Read-only
/// guarantees come from the database account, not from this definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DatabaseQueryFields")]
pub struct DatabaseQuery {
  credentials: SecretId,
  query: String,
}

#[derive(Deserialize)]
struct DatabaseQueryFields {
  credentials: SecretId,
  query: String,
}

impl TryFrom<DatabaseQueryFields> for DatabaseQuery {
  type Error = ToolError;

  fn try_from(fields: DatabaseQueryFields) -> Result<Self, ToolError> {
    Self::new(fields.credentials, fields.query)
  }
}

impl DatabaseQuery {
  pub fn new(credentials: SecretId, query: String) -> Result<Self, ToolError> {
    if query.trim().is_empty() {
      return Err(ToolError::BlankQuery);
    }
    Ok(Self { credentials, query })
  }

  pub const fn credentials(&self) -> SecretId {
    self.credentials
  }

  pub fn query(&self) -> &str {
    &self.query
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAction {
  Http(HttpRequest),
  Command(CommandAction),
  DatabaseQuery(DatabaseQuery),
}

/// The kinds of tool the library offers to create (06.01, "Create a tool").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
  Http,
  Command,
  Database,
}

impl ToolKind {
  /// Whether a pipeline step can run a tool of this kind yet. HTTP and database tools use project
  /// secrets and runtime approvals, which do not exist yet; command tools hold no secrets.
  pub const fn runnable(self) -> bool {
    matches!(self, Self::Command)
  }
}

impl ToolAction {
  pub const fn kind(&self) -> ToolKind {
    match self {
      Self::Http(_) => ToolKind::Http,
      Self::Command(_) => ToolKind::Command,
      Self::DatabaseQuery(_) => ToolKind::Database,
    }
  }
}

/// A concrete callable action, what it is for, and the policy that governs calls to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Tool {
  /// What the tool does, as the user wrote it; may be empty.
  #[serde(skip_serializing_if = "String::is_empty")]
  description: String,
  action: ToolAction,
  #[serde(skip_serializing_if = "Option::is_none")]
  access: Option<AccessPolicy>,
}

#[derive(Deserialize)]
struct ToolFields {
  #[serde(default)]
  description: String,
  action: ToolAction,
  #[serde(default)]
  access: Option<AccessPolicy>,
}

impl<'de> Deserialize<'de> for Tool {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let fields = ToolFields::deserialize(deserializer)?;
    Self::new(&fields.description, fields.action, fields.access).map_err(serde::de::Error::custom)
  }
}

impl Tool {
  /// A tool that runs `action`. HTTP and database tools need an access policy. A command tool
  /// holds no secret and needs none: without one, any step of a project pipeline that names it
  /// may run it; with one, only the steps it allows.
  pub fn new(description: &str, action: ToolAction, access: Option<AccessPolicy>) -> Result<Self, ToolError> {
    if access.is_none() && action.kind() != ToolKind::Command {
      return Err(ToolError::AccessPolicyRequired);
    }
    Ok(Self {
      description: description.trim().to_owned(),
      action,
      access,
    })
  }

  /// A command tool without an access policy.
  pub fn command(description: &str, action: CommandAction) -> Self {
    Self {
      description: description.trim().to_owned(),
      action: ToolAction::Command(action),
      access: None,
    }
  }

  pub fn description(&self) -> &str {
    &self.description
  }

  pub const fn action(&self) -> &ToolAction {
    &self.action
  }

  pub const fn kind(&self) -> ToolKind {
    self.action.kind()
  }

  pub const fn access(&self) -> Option<&AccessPolicy> {
    self.access.as_ref()
  }
}

/// Converts a tool body of schema 1 (`{action, access}`, a command being only `{program, args}`) to
/// schema 2: the command gets the defaults of [`CommandAction::new`]; everything else is kept.
pub(super) fn upgrade_from_schema_1(mut body: serde_json::Value) -> Option<serde_json::Value> {
  if let Some(command) = body
    .pointer_mut("/action/command")
    .and_then(serde_json::Value::as_object_mut)
  {
    let defaults = [
      ("timeout_secs", serde_json::json!(CommandTimeout::DEFAULT.secs())),
      ("expected_exit_codes", serde_json::json!([0])),
      ("environment", serde_json::json!({})),
      ("output_limit_kib", serde_json::json!(OutputLimit::DEFAULT.kib())),
    ];
    for (field, value) in defaults {
      command.entry(field).or_insert(value);
    }
  }
  body.is_object().then_some(body)
}
