//! The small part of Codex App Server's JSON-RPC stream an agent step needs. The parser keeps
//! tool arguments out of activity and error output; only an explicit file path may become a
//! request target. Unknown server requests fail closed instead of leaving a turn hanging.

use std::{collections::HashMap, fmt};

use serde_json::{Value, json};
use thiserror::Error;

use super::stream::{AgentEvent, CommandStatus};
use crate::run::TokenUsage;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
  #[error("Codex App Server sent invalid JSON")]
  InvalidJson,
  #[error("Codex App Server requested unsupported input: {method}")]
  UnsupportedRequest { method: String },
  #[error("Codex App Server sent an approval request without a usable decision")]
  InvalidApproval,
}

/// One host decision for one pending JSON-RPC request. Arbitrary tool arguments and form
/// contents are deliberately absent from its debug representation and persisted display data.
#[derive(Clone, PartialEq)]
pub struct PendingApproval {
  id: Value,
  kind: ApprovalKind,
  pub thread: String,
  pub turn: Option<String>,
  pub server: String,
  pub action: String,
  pub target: Option<String>,
  pub scope: Option<String>,
}

impl fmt::Debug for PendingApproval {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("PendingApproval")
      .field("thread", &self.thread)
      .field("server", &self.server)
      .finish_non_exhaustive()
  }
}

#[derive(Clone, PartialEq)]
enum ApprovalKind {
  Command {
    denial: CommandDenial,
  },
  UserInput {
    question: String,
    accept: String,
    decline: String,
  },
  Elicitation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommandDenial {
  Decline,
  Cancel,
}

impl PendingApproval {
  pub fn is_command(&self) -> bool {
    matches!(self.kind, ApprovalKind::Command { .. })
  }

  /// Stable within one invocation; the invocation ID is prefixed by the runner for storage.
  pub fn request_key(&self) -> String {
    self.id.to_string()
  }

  /// JSON-RPC response for exactly this request. The approval never persists to later calls.
  pub fn response(&self, approve: bool) -> String {
    let result = match &self.kind {
      ApprovalKind::Command { denial } => {
        let decision = match (approve, denial) {
          (true, _) => "accept",
          (false, CommandDenial::Decline) => "decline",
          (false, CommandDenial::Cancel) => "cancel",
        };
        json!({"decision": decision})
      }
      ApprovalKind::UserInput {
        question,
        accept,
        decline,
      } => {
        let chosen = if approve { accept } else { decline };
        json!({"answers": {question: {"answers": [chosen]}}})
      }
      ApprovalKind::Elicitation if approve => json!({"action": "accept", "content": {}, "_meta": null}),
      ApprovalKind::Elicitation => json!({"action": "decline", "content": null, "_meta": null}),
    };
    json!({"id": self.id, "result": result}).to_string()
  }
}

/// Progress from a single App Server line.
pub enum Update {
  Activity(AgentEvent),
  Approval(PendingApproval),
  TurnEnded,
}

#[derive(Clone)]
struct ToolContext {
  thread: String,
  turn: String,
  server: String,
  action: String,
  target: Option<String>,
}

/// Parses notifications and correlates a later approval request with its pending tool item.
#[derive(Default)]
pub struct AppServerStream {
  tools: HashMap<String, ToolContext>,
  usage: TokenUsage,
}

impl AppServerStream {
  /// A started MCP action remains active until its completion notification arrives.
  pub fn has_active_tools(&self) -> bool {
    !self.tools.is_empty()
  }

  pub fn parse(&mut self, line: &str) -> Result<Vec<Update>, ProtocolError> {
    let message: Value = serde_json::from_str(line).map_err(|_| ProtocolError::InvalidJson)?;
    let Some(method) = message.get("method").and_then(Value::as_str) else {
      return Ok(Vec::new());
    };
    let params = message.get("params").unwrap_or(&Value::Null);
    if let Some(id) = message.get("id") {
      return self
        .request(method, id, params)
        .map(|request| vec![Update::Approval(request)]);
    }
    Ok(self.notification(method, params))
  }

  fn request(&self, method: &str, id: &Value, params: &Value) -> Result<PendingApproval, ProtocolError> {
    match method {
      "item/tool/requestUserInput" | "tool/requestUserInput" => self.user_input(id, params),
      "mcpServer/elicitation/request" => self.elicitation(id, params),
      "item/commandExecution/requestApproval" => Self::command_approval(id, params),
      _ => Err(ProtocolError::UnsupportedRequest {
        method: method.to_owned(),
      }),
    }
  }

  fn command_approval(id: &Value, params: &Value) -> Result<PendingApproval, ProtocolError> {
    // Additional permission overlays can remain sticky for later calls in a turn. Never grant them.
    if params
      .get("additionalPermissions")
      .is_some_and(|value| !value.is_null())
      || params
        .get("environmentId")
        .is_some_and(|value| !value.is_null() && value.as_str() != Some("local"))
      || params
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind != "command")
    {
      return Err(ProtocolError::InvalidApproval);
    }
    string(params, "itemId").ok_or(ProtocolError::InvalidApproval)?;
    Ok(PendingApproval {
      id: id.clone(),
      kind: ApprovalKind::Command {
        denial: command_denial(params)?,
      },
      thread: string(params, "threadId").ok_or(ProtocolError::InvalidApproval)?,
      turn: Some(string(params, "turnId").ok_or(ProtocolError::InvalidApproval)?),
      server: "command".to_owned(),
      action: string(params, "command").ok_or(ProtocolError::InvalidApproval)?,
      target: Some(string(params, "cwd").ok_or(ProtocolError::InvalidApproval)?),
      scope: string(params, "reason"),
    })
  }

  fn user_input(&self, id: &Value, params: &Value) -> Result<PendingApproval, ProtocolError> {
    if params.get("isBlocking").and_then(Value::as_bool) != Some(true) {
      return Err(ProtocolError::InvalidApproval);
    }
    let questions = params
      .get("questions")
      .and_then(Value::as_array)
      .ok_or(ProtocolError::InvalidApproval)?;
    let [question] = questions.as_slice() else {
      return Err(ProtocolError::InvalidApproval);
    };
    if question.get("isSecret").and_then(Value::as_bool).unwrap_or(false)
      || question.get("isOther").and_then(Value::as_bool).unwrap_or(false)
    {
      return Err(ProtocolError::InvalidApproval);
    }
    let options = question
      .get("options")
      .and_then(Value::as_array)
      .ok_or(ProtocolError::InvalidApproval)?;
    let label = |names: &[&str]| -> Option<String> {
      options
        .iter()
        .filter_map(|option| option.get("label").and_then(Value::as_str))
        .find_map(|value| {
          names
            .iter()
            .any(|name| value.eq_ignore_ascii_case(name))
            .then(|| value.to_owned())
        })
    };
    let accept =
      label(&["Accept", "Approve", "Approve once", "Allow once", "Allow"]).ok_or(ProtocolError::InvalidApproval)?;
    let decline = label(&["Decline", "Deny", "Reject", "Cancel"]).ok_or(ProtocolError::InvalidApproval)?;
    let item = string(params, "itemId").ok_or(ProtocolError::InvalidApproval)?;
    let context = self.tools.get(&item).ok_or(ProtocolError::InvalidApproval)?;
    let thread = string(params, "threadId").ok_or(ProtocolError::InvalidApproval)?;
    let turn = string(params, "turnId").ok_or(ProtocolError::InvalidApproval)?;
    if context.thread != thread || context.turn != turn {
      return Err(ProtocolError::InvalidApproval);
    }
    Ok(PendingApproval {
      id: id.clone(),
      kind: ApprovalKind::UserInput {
        question: string(question, "id").ok_or(ProtocolError::InvalidApproval)?,
        accept,
        decline,
      },
      thread,
      turn: Some(turn),
      server: context.server.clone(),
      action: context.action.clone(),
      target: context.target.clone(),
      scope: None,
    })
  }

  fn elicitation(&self, id: &Value, params: &Value) -> Result<PendingApproval, ProtocolError> {
    let mode = params
      .get("mode")
      .and_then(Value::as_str)
      .ok_or(ProtocolError::InvalidApproval)?;
    let schema = params.get("requestedSchema").ok_or(ProtocolError::InvalidApproval)?;
    let required = schema.get("required").and_then(Value::as_array);
    let properties = schema.get("properties").and_then(Value::as_object);
    if mode != "form"
      || required.is_some_and(|fields| !fields.is_empty())
      || properties.is_some_and(|fields| !fields.is_empty())
    {
      return Err(ProtocolError::InvalidApproval);
    }
    let server = string(params, "serverName").ok_or(ProtocolError::InvalidApproval)?;
    let thread = string(params, "threadId").ok_or(ProtocolError::InvalidApproval)?;
    let turn = string(params, "turnId").ok_or(ProtocolError::InvalidApproval)?;
    let mut matching = self
      .tools
      .values()
      .filter(|tool| tool.server == server && tool.thread == thread && tool.turn == turn);
    let tool = matching.next().ok_or(ProtocolError::InvalidApproval)?;
    if matching.next().is_some() {
      return Err(ProtocolError::InvalidApproval);
    }
    Ok(PendingApproval {
      id: id.clone(),
      kind: ApprovalKind::Elicitation,
      thread,
      turn: Some(turn),
      server,
      action: tool.action.clone(),
      target: tool.target.clone(),
      scope: None,
    })
  }

  fn notification(&mut self, method: &str, params: &Value) -> Vec<Update> {
    // The runner records the session from the thread/start or thread/resume response;
    // thread/started also arrives for new threads and must not duplicate that activity.
    match method {
      "turn/started" => vec![Update::Activity(AgentEvent::TurnStarted)],
      "item/started" => self.item_started(params),
      "item/completed" => self.item_completed(params),
      "thread/tokenUsage/updated" => {
        if let Some(last) = params.get("tokenUsage").and_then(|value| value.get("last")) {
          self.usage = usage(last);
        }
        Vec::new()
      }
      "turn/completed" => self.turn_completed(params),
      "error" => string(params.get("error").unwrap_or(&Value::Null), "message")
        .map(|message| vec![Update::Activity(AgentEvent::Error { message })])
        .unwrap_or_default(),
      _ => Vec::new(),
    }
  }

  fn item_started(&mut self, params: &Value) -> Vec<Update> {
    let Some(item) = params.get("item") else {
      return Vec::new();
    };
    match item.get("type").and_then(Value::as_str) {
      Some("mcpToolCall") => {
        if let (Some(id), Some(thread), Some(turn), Some(server), Some(action)) = (
          string(item, "id"),
          string(params, "threadId"),
          string(params, "turnId"),
          string(item, "server"),
          string(item, "tool"),
        ) {
          let context = ToolContext {
            thread,
            turn,
            server,
            action,
            target: item.get("arguments").and_then(|args| string(args, "filePath")),
          };
          self.tools.insert(id, context);
        }
        Vec::new()
      }
      Some("commandExecution") => match (string(item, "id"), string(item, "command")) {
        (Some(item), Some(command)) => vec![Update::Activity(AgentEvent::CommandStarted { item, command })],
        _ => Vec::new(),
      },
      _ => Vec::new(),
    }
  }

  fn item_completed(&mut self, params: &Value) -> Vec<Update> {
    let Some(item) = params.get("item") else {
      return Vec::new();
    };
    match item.get("type").and_then(Value::as_str) {
      Some("mcpToolCall") => {
        if let Some(id) = string(item, "id") {
          self.tools.remove(&id);
        }
        Vec::new()
      }
      Some("agentMessage") => string(item, "text")
        .map(|text| vec![Update::Activity(AgentEvent::Message { text })])
        .unwrap_or_default(),
      Some("reasoning") => item
        .get("summary")
        .and_then(Value::as_array)
        .map(|parts| parts.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"))
        .filter(|text| !text.is_empty())
        .map(|text| vec![Update::Activity(AgentEvent::Reasoning { text })])
        .unwrap_or_default(),
      Some("commandExecution") => command_finished(item),
      _ => Vec::new(),
    }
  }

  fn turn_completed(&mut self, params: &Value) -> Vec<Update> {
    let turn = params.get("turn").unwrap_or(&Value::Null);
    let mut updates = vec![Update::Activity(AgentEvent::TurnCompleted { usage: self.usage })];
    self.usage = TokenUsage::default();
    if turn.get("status").and_then(Value::as_str) != Some("completed") {
      let message = turn
        .get("error")
        .and_then(|error| string(error, "message"))
        .unwrap_or_else(|| "Codex turn did not complete".to_owned());
      updates.push(Update::Activity(AgentEvent::TurnFailed { message }));
    }
    updates.push(Update::TurnEnded);
    updates
  }
}

fn command_denial(params: &Value) -> Result<CommandDenial, ProtocolError> {
  let decisions = match params.get("availableDecisions") {
    None | Some(Value::Null) => return Ok(CommandDenial::Decline),
    Some(Value::Array(decisions)) => decisions,
    Some(_) => return Err(ProtocolError::InvalidApproval),
  };
  let available = |name: &str| decisions.iter().any(|decision| decision.as_str() == Some(name));
  if !available("accept") {
    return Err(ProtocolError::InvalidApproval);
  }
  // Rule-based prompts can offer cancel rather than decline. Both deny this action;
  // cancel also interrupts the turn. Never choose a session or policy amendment grant.
  if available("decline") {
    Ok(CommandDenial::Decline)
  } else if available("cancel") {
    Ok(CommandDenial::Cancel)
  } else {
    Err(ProtocolError::InvalidApproval)
  }
}

fn command_finished(item: &Value) -> Vec<Update> {
  let status = match item.get("status").and_then(Value::as_str) {
    Some("completed") => CommandStatus::Completed,
    Some("failed") => CommandStatus::Failed,
    Some("declined") => CommandStatus::Declined,
    _ => return Vec::new(),
  };
  match (string(item, "id"), string(item, "command")) {
    (Some(item_id), Some(command)) => vec![Update::Activity(AgentEvent::CommandFinished {
      item: item_id,
      command,
      output: string(item, "aggregatedOutput").unwrap_or_default(),
      exit_code: item.get("exitCode").and_then(Value::as_i64),
      status,
    })],
    _ => Vec::new(),
  }
}

fn usage(value: &Value) -> TokenUsage {
  TokenUsage {
    input: value.get("inputTokens").and_then(Value::as_u64).unwrap_or(0),
    cached_input: value.get("cachedInputTokens").and_then(Value::as_u64).unwrap_or(0),
    output: value.get("outputTokens").and_then(Value::as_u64).unwrap_or(0),
  }
}

fn string(value: &Value, key: &str) -> Option<String> {
  value.get(key).and_then(Value::as_str).map(str::to_owned)
}
