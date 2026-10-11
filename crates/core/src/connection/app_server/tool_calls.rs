//! The MCP calls Codex reports: what the stream keeps of a call from its start to its end, and the calls of
//! Orchester's tools the runner asked a person about itself. Codex runs Orchester's tools without asking (its
//! configuration approves them), so the runner asks before it runs a call that needs an approval; the call's end then
//! says a person was asked, as for a call Codex asked about. The runner's request and Codex's start of the call come
//! on different channels, so either may be seen first: a request whose call has not started yet is kept until it
//! does, and forgotten when the turn ends.

use serde_json::Value;

use super::{AppServerStream, Update, string};
use crate::{
  connection::stream::{AgentEvent, FileChange, NativeCall},
  orchester_tools::{OrchesterTool, SERVER as ORCHESTER},
};

/// A started MCP call, until it ends.
#[derive(Clone)]
pub(super) struct ToolContext {
  pub(super) thread: String,
  pub(super) turn: String,
  pub(super) server: String,
  pub(super) action: String,
  pub(super) target: Option<String>,
  /// Codex asked for approval of the call, or the runner did for a call of Orchester's tools.
  pub(super) asked: bool,
  /// The arguments of a call of Orchester's tools, to find the call the runner asks about; `Null` for any other.
  arguments: Value,
  /// The files the call changes when it succeeds: a call of Orchester's `edit_file`, `create_file`, `delete_file` or
  /// `move_file`.
  pub(super) changes: Vec<FileChange>,
  /// What a call of one of Orchester's file tools asked, for its row.
  pub(super) call: Option<NativeCall>,
}

/// The runner's requests about calls of Orchester's tools that have not started yet: the tool and its arguments.
#[derive(Default)]
pub(super) struct AskedOwnCalls(Vec<(String, Value)>);

impl AskedOwnCalls {
  fn remember(&mut self, action: &str, arguments: &Value) {
    self.0.push((action.to_owned(), arguments.clone()));
  }

  /// Whether the runner asked about a call of `action` with `arguments`, forgetting that request.
  fn take(&mut self, action: &str, arguments: &Value) -> bool {
    let found = self
      .0
      .iter()
      .position(|(asked, input)| asked == action && input == arguments);
    found.map(|at| self.0.remove(at)).is_some()
  }

  /// Forgets every request: at a turn's end, a call not started yet never will be.
  pub(super) fn clear(&mut self) {
    self.0.clear();
  }
}

impl AppServerStream {
  /// Orchester calls can outlive a yielded code-mode exec and the model turn that started it.
  pub fn has_active_orchester_tools(&self) -> bool {
    self.tools.values().any(|tool| tool.server == ORCHESTER)
  }
  /// The runner asks a person about a call of Orchester's tool `action` with `arguments`: the call's end says a person
  /// was asked. The id of the call when it has started, so the request and the call are one row; `None` when it has
  /// not, and its start is marked then (within the same turn).
  pub fn runner_asks(&mut self, action: &str, arguments: &Value) -> Option<String> {
    let started = self.tools.iter_mut().find(|(_, tool)| {
      tool.server == ORCHESTER && tool.action == action && !tool.asked && tool.arguments == *arguments
    });
    match started {
      Some((item, tool)) => {
        tool.asked = true;
        Some(item.clone())
      }
      None => {
        self.asked_own.remember(action, arguments);
        None
      }
    }
  }

  /// The start of the MCP call `item`: kept until its end; a call of a tool that runs without asking is reported as not
  /// asked.
  pub(super) fn mcp_tool_started(&mut self, params: &Value, item: &Value) -> Vec<Update> {
    let (Some(id), Some(thread), Some(turn), Some(server), Some(action)) = (
      string(item, "id"),
      string(params, "threadId"),
      string(params, "turnId"),
      string(item, "server"),
      string(item, "tool"),
    ) else {
      return Vec::new();
    };
    let not_asked = self
      .exemptions
      .exempts(&server, &action)
      .then(|| AgentEvent::McpToolNotAsked {
        item: id.clone(),
        server: server.clone(),
        tool: action.clone(),
      });
    let arguments = item.get("arguments").unwrap_or(&Value::Null);
    let own = (server == ORCHESTER)
      .then(|| OrchesterTool::from_name(&action))
      .flatten();
    let context = ToolContext {
      thread,
      turn,
      target: string(arguments, "filePath"),
      asked: own.is_some() && self.asked_own.take(&action, arguments),
      arguments: if own.is_some() { arguments.clone() } else { Value::Null },
      changes: own.map_or_else(Vec::new, |tool| tool.changed_files(arguments)),
      call: own.and_then(|tool| tool.shown_call(arguments)),
      server,
      action,
    };
    self.tools.insert(id, context);
    not_asked.map(Update::Activity).into_iter().collect()
  }
}
