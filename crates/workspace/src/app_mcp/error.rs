//! Why a tool did not do what it was asked, as the client reads it: a stable `code` to match on, and
//! the reasons in the words the app shows a person for the same refusal.

use orchester_ipc::Rejection;
use orchester_store::StoreError;
use serde_json::{Value, json};

use crate::{
  messages::{self, Message},
  runner::{RequestError, RunRequestError},
};

/// What a tool names and Orchester does not know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
  Project,
  Task,
  Run,
  Approval,
  Question,
  Pipeline,
  Profile,
  Step,
  Connection,
  Instruction,
  Skill,
  Tool,
  McpServer,
  Account,
}

impl Missing {
  const fn code(self) -> &'static str {
    match self {
      Self::Project => "project_not_found",
      Self::Task => "task_not_found",
      Self::Run => "run_not_found",
      Self::Approval => "approval_not_found",
      Self::Question => "question_not_found",
      Self::Pipeline => "pipeline_not_found",
      Self::Profile => "profile_not_found",
      Self::Step => "step_not_found",
      Self::Connection => "connection_not_found",
      Self::Instruction => "instruction_not_found",
      Self::Skill => "skill_not_found",
      Self::Tool => "tool_not_found",
      Self::McpServer => "mcp_server_not_found",
      Self::Account => "account_not_found",
    }
  }
}

#[derive(Debug)]
pub enum ToolError {
  /// The arguments do not match the tool's input schema.
  InvalidArguments(String),
  /// Nothing answers to `named`.
  NotFound { missing: Missing, named: String },
  /// More than one project answers to the name; the client names it by id.
  AmbiguousProject(String),
  /// Orchester refused, as it refuses a person in the app: `code` names the refusal.
  Refused { code: String, reasons: Vec<Message> },
}

impl ToolError {
  pub fn not_found(missing: Missing, named: impl Into<String>) -> Self {
    Self::NotFound {
      missing,
      named: named.into(),
    }
  }

  pub fn refused(code: impl Into<String>, reasons: Vec<Message>) -> Self {
    Self::Refused {
      code: code.into(),
      reasons,
    }
  }

  /// Work handed to the blocking pool did not finish, e.g. while the app closes.
  pub fn interrupted() -> Self {
    Self::refused("interrupted", vec![messages::interrupted()])
  }

  /// The runner did not do what was asked: the rejection's own name, or why it was not asked.
  pub fn runner(error: &RunRequestError, reasons: Vec<Message>) -> Self {
    let code = match error {
      RunRequestError::Request(RequestError::NotConnected) => "runner_unavailable".to_owned(),
      RunRequestError::Request(RequestError::Lost) => "runner_reply_lost".to_owned(),
      RunRequestError::Rejected(rejection) => rejection_code(rejection),
    };
    Self::refused(code, reasons)
  }

  /// The error as the tool result carries it, its reasons in `translate`d words.
  pub fn to_json(&self, translate: impl Fn(&Message) -> String) -> Value {
    let (code, message, reasons) = match self {
      Self::InvalidArguments(detail) => ("invalid_arguments".to_owned(), detail.clone(), Vec::new()),
      Self::NotFound { missing, named } => (
        missing.code().to_owned(),
        format!("nothing answers to {named:?}"),
        Vec::new(),
      ),
      Self::AmbiguousProject(named) => (
        "project_ambiguous".to_owned(),
        format!("more than one project is named {named:?}; name it by id"),
        Vec::new(),
      ),
      Self::Refused { code, reasons } => {
        let reasons: Vec<String> = reasons.iter().map(&translate).collect();
        (code.clone(), reasons.join("\n"), reasons)
      }
    };
    json!({ "error": { "code": code, "message": message, "reasons": reasons } })
  }
}

impl From<StoreError> for ToolError {
  fn from(error: StoreError) -> Self {
    tracing::warn!(%error, "an MCP tool met a project data error");
    let code = match &error {
      StoreError::Organization(orchester_core::task::organization::OrganizationError::StopRunFirst(_)) => {
        "stop_run_first"
      }
      StoreError::Organization(_) => "invalid_organization",
      StoreError::TaskNotEditable { .. } => "task_not_editable",
      StoreError::SharedTask(_) | StoreError::SharedProject | StoreError::NotSharedTask(_) => "shared_task",
      StoreError::PipelineNotPublished(_) => "pipeline_not_published",
      _ => "project_data",
    };
    Self::refused(code, vec![messages::store_error(&error)])
  }
}

/// A runner rejection's name, as the protocol writes it (`task_not_queued`, `run_not_running`…).
fn rejection_code(rejection: &Rejection) -> String {
  let value = serde_json::to_value(rejection).expect("a rejection is plain data with string keys");
  value
    .get("reason")
    .and_then(Value::as_str)
    .map_or_else(|| "rejected".to_owned(), str::to_owned)
}
