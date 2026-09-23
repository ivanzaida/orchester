mod access;
mod http;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  access::{AccessPolicy, AccessPolicyError, AccessWindow, AllowedSteps, ApprovalMode, DeniedBehavior},
  http::{FieldName, FieldNameError, HttpAuth, HttpError, HttpMethod, HttpRequest, RequestTarget, ResponseSource},
};
use crate::id::SecretId;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolError {
  #[error("a command tool needs a program")]
  BlankProgram,
  #[error("a database tool needs a query")]
  BlankQuery,
}

/// A program run with fixed arguments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CommandFields")]
pub struct CommandAction {
  program: String,
  args: Vec<String>,
}

#[derive(Deserialize)]
struct CommandFields {
  program: String,
  args: Vec<String>,
}

impl TryFrom<CommandFields> for CommandAction {
  type Error = ToolError;

  fn try_from(fields: CommandFields) -> Result<Self, ToolError> {
    Self::new(fields.program, fields.args)
  }
}

impl CommandAction {
  pub fn new(program: String, args: Vec<String>) -> Result<Self, ToolError> {
    if program.trim().is_empty() {
      return Err(ToolError::BlankProgram);
    }
    Ok(Self { program, args })
  }

  pub fn program(&self) -> &str {
    &self.program
  }

  pub fn args(&self) -> &[String] {
    &self.args
  }
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

/// A concrete callable action and the policy that governs calls to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
  action: ToolAction,
  access: AccessPolicy,
}

impl Tool {
  pub const fn new(action: ToolAction, access: AccessPolicy) -> Self {
    Self { action, access }
  }

  pub const fn action(&self) -> &ToolAction {
    &self.action
  }

  pub const fn access(&self) -> &AccessPolicy {
    &self.access
  }
}
