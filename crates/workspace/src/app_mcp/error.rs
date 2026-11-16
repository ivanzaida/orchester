//! Why a tool did not do what it was asked, as the client reads it: a stable `code` to match on, and
//! the reasons in the words the app shows a person for the same refusal.

use orchester_ipc::Rejection;
use orchester_store::StoreError;
use serde_json::{Value, json};

use crate::{
  messages::{self, Message},
  readiness::{NotReadyHere, ReadinessError},
  runner::{RequestError, RunRequestError},
  tasks::team_pipelines::PipelinesUnread,
  team_config::TeamNotRead,
};

/// What a tool names and Orchester does not know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
  Project,
  Task,
  Run,
  Approval,
  Question,
  Plan,
  Pipeline,
  Profile,
  Reviewer,
  Step,
  Connection,
  Instruction,
  Skill,
  Tool,
  McpServer,
  Account,
  Module,
  Service,
  Stack,
  /// An Orchestrator conversation of the project.
  Conversation,
}

impl Missing {
  const fn code(self) -> &'static str {
    match self {
      Self::Project => "project_not_found",
      Self::Task => "task_not_found",
      Self::Run => "run_not_found",
      Self::Approval => "approval_not_found",
      Self::Question => "question_not_found",
      Self::Plan => "plan_not_found",
      Self::Pipeline => "pipeline_not_found",
      Self::Profile => "profile_not_found",
      Self::Reviewer => "reviewer_not_found",
      Self::Step => "step_not_found",
      Self::Connection => "connection_not_found",
      Self::Instruction => "instruction_not_found",
      Self::Skill => "skill_not_found",
      Self::Tool => "tool_not_found",
      Self::McpServer => "mcp_server_not_found",
      Self::Account => "account_not_found",
      Self::Module => "module_not_found",
      Self::Service => "service_not_found",
      Self::Stack => "stack_not_found",
      Self::Conversation => "conversation_not_found",
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
  /// Orchester refused, and says what the person would confirm: `preview` is what a client reads typed (an archive of a
  /// team resource names what uses it and the runs that would stop).
  Previewed {
    code: String,
    reasons: Vec<Message>,
    preview: Value,
  },
  /// A start that starts nothing because its pipeline is not ready on this computer (`not_ready_here`): the parts, each
  /// with its fix, which a client reads typed.
  NotReadyHere(NotReadyHere),
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

  /// A refusal that carries `preview`, what the person would confirm.
  pub fn previewed(code: impl Into<String>, reasons: Vec<Message>, preview: Value) -> Self {
    Self::Previewed {
      code: code.into(),
      reasons,
      preview,
    }
  }

  /// Work handed to the blocking pool did not finish, e.g. while the app closes.
  pub fn interrupted() -> Self {
    Self::refused("interrupted", vec![messages::interrupted()])
  }

  /// What is named waits for an Orchester Cloud sign-in, which only a person makes, in the window.
  pub fn sign_in_required() -> Self {
    Self::refused("sign_in_required", vec![Message::new("appMcp.errors.signInRequired")])
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

  /// The stable code a client matches on.
  pub fn code(&self) -> &str {
    match self {
      Self::InvalidArguments(_) => "invalid_arguments",
      Self::NotFound { missing, .. } => missing.code(),
      Self::AmbiguousProject(_) => "project_ambiguous",
      Self::Refused { code, .. } | Self::Previewed { code, .. } => code,
      Self::NotReadyHere(_) => "not_ready_here",
    }
  }

  /// The error as the tool result carries it, its reasons in `translate`d words.
  pub fn to_json(&self, translate: impl Fn(&Message) -> String) -> Value {
    let (message, reasons) = match self {
      Self::InvalidArguments(detail) => (detail.clone(), Vec::new()),
      Self::NotFound { named, .. } => (format!("nothing answers to {named:?}"), Vec::new()),
      Self::AmbiguousProject(named) => (
        format!("more than one project is named {named:?}; name it by id"),
        Vec::new(),
      ),
      Self::Refused { reasons, .. } => {
        let reasons: Vec<String> = reasons.iter().map(&translate).collect();
        (reasons.join("\n"), reasons)
      }
      Self::Previewed { reasons, preview, .. } => {
        let reasons: Vec<String> = reasons.iter().map(&translate).collect();
        let error = json!({
          "code": self.code(),
          "message": reasons.join("\n"),
          "reasons": reasons,
          "preview": preview,
        });
        return json!({ "error": error });
      }
      Self::NotReadyHere(refusal) => {
        let reasons: Vec<String> = refusal.messages().iter().map(&translate).collect();
        let error = json!({
          "code": self.code(),
          "message": reasons.join("\n"),
          "reasons": reasons,
          "parts": refusal.parts_json(),
        });
        return json!({ "error": error });
      }
    };
    json!({ "error": { "code": self.code(), "message": message, "reasons": reasons } })
  }
}

impl From<ReadinessError> for ToolError {
  /// A team project's configuration that is being read, or could not be read from its team server this session, is
  /// `team_reading` or `team_unreachable` with the reason; any other failure `readiness_unreadable`.
  fn from(error: ReadinessError) -> Self {
    let code = match &error {
      ReadinessError::TeamReading => "team_reading",
      ReadinessError::TeamUnreachable(_) => "team_unreachable",
      ReadinessError::Store(_) | ReadinessError::NotATeamProject => {
        tracing::warn!(%error, "the readiness of a project could not be read for a tool");
        "readiness_unreadable"
      }
    };
    Self::refused(code, vec![error.message()])
  }
}

impl From<TeamNotRead> for ToolError {
  /// A team project's configuration that is being read, or could not be read from its team server this session:
  /// `team_reading` or `team_unreachable`, with the reason. Nothing older is read in its place.
  fn from(error: TeamNotRead) -> Self {
    let code = match &error {
      TeamNotRead::Reading => "team_reading",
      TeamNotRead::Unreachable(_) => "team_unreachable",
    };
    Self::refused(code, vec![error.message()])
  }
}

impl From<PipelinesUnread> for ToolError {
  fn from(error: PipelinesUnread) -> Self {
    match error {
      PipelinesUnread::Store(error) => error.into(),
      PipelinesUnread::Team(error) => error.into(),
    }
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
      StoreError::Module(orchester_core::module::ModuleError::NotFound(_)) => "module_not_found",
      StoreError::Module(orchester_core::module::ModuleError::ChangedMeanwhile) => "module_changed",
      StoreError::Module(_) => "invalid_module",
      StoreError::TaskNotEditable { .. } => "task_not_editable",
      StoreError::SharedTask(_) | StoreError::SharedProject | StoreError::NotSharedTask(_) => "shared_task",
      StoreError::NotAdmin => "not_admin",
      StoreError::PipelineNotPublished(_) => "pipeline_not_published",
      StoreError::DraftChanged(_) => "draft_changed",
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
