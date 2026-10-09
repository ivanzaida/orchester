//! `claude -p --output-format stream-json --verbose` output: one JSON object per line
//! (`.tmp/cli-probe/REPORT.md`). `system` with subtype `init` carries the session id;
//! `assistant{message{id, content[], usage}, error?}` the agent's text, thinking and tool calls;
//! `user{message{content[]}}` the results of those tool calls; `result{is_error, result, usage,
//! permission_denials[]}` ends the invocation. A failed sign-in is an assistant message with
//! `error: "authentication_failed"` and a result with `is_error` (its subtype still says "success").
//!
//! The CLI can print one assistant message as several lines, one per content block, each repeating
//! the message's usage; usage is therefore counted once per message id, as the growth over what
//! that message already reported. The result's usage covers the whole invocation; whatever it holds
//! beyond the messages is counted when it arrives. Claude reports cache reads and cache writes apart
//! from `input_tokens`: reads are cached input, which does not count; writes count as input
//! (`docs/design/orchester-workspace.md`, "Token budgets"). `total_cost_usd` is ignored.
//!
//! The step result ([`StepResult`]) is the result's `structured_output`, which the CLI fills when it
//! was given the result schema (`--json-schema`); else the one that closes the final message
//! ([`StepResult::at_end`]). Either becomes the invocation's last message, as a JSON document. A
//! turn that ends without one fails: the prompt asks for it, and prose alone must never pass a step
//! that the agent may have meant to fail or to ask about.
//!
//! The prompt and a person's messages sent now arrive as stream-json user messages on standard
//! input; with `--replay-user-messages` the CLI prints each back (`isReplay`) as it takes it into
//! the conversation. Claude Code 2.1.286 (probe 2026-10-02) takes a message written during a tool
//! call right after that call's result, and one written after its last tool call as the next turn,
//! after the result ([`ClaudeStream::take_replayed`], [`ClaudeStream::results`]).
//!
//! An edit with Claude Code's own tools ([`EDIT_TOOLS`]) is reported as the change it made once its result says
//! it happened. A call of Orchester's `edit_file` or `create_file` (`mcp__Orchester__edit_file`, served over the
//! runner's relay) is a command like any MCP call, and its change follows its successful end, so that the first
//! edit is seen whichever tools the agent edits with. What a call of Orchester's file tools asked follows its start
//! ([`AgentEvent::NativeCall`]): its row reads as the native agent's call of the tool, an edit's with its diff.
//!
//! A result before the CLI took the prompt, saying nothing, ends no turn of the agent's: Claude Code resuming a
//! session whose background command the previous process left reports that command stopped, ends a turn of its own
//! with such a result, then starts again and takes the prompt (run 22, 2026-10-06). It neither fails the step nor
//! counts as a turn the CLI ended.

use std::collections::HashMap;

use serde_json::Value;

use super::{AgentEvent, CommandStatus, FileChange, FileChangeKind, OutputStream};
use crate::{
  connection::mcp::{InvocationExemptions, McpServerProblem},
  orchester_tools::OrchesterTool,
  run::{StepResult, TokenUsage},
};

/// The `error` of an assistant message whose CLI is not signed in.
const AUTHENTICATION_FAILED: &str = "authentication_failed";
/// Content blocks of an assistant message the parser reads; `redacted_thinking` carries nothing a
/// person can read.
const KNOWN_BLOCKS: [&str; 4] = ["text", "thinking", "redacted_thinking", "tool_use"];
/// Why a turn that ended without a step result failed.
const NO_RESULT: &str = "Claude Code ended its turn without the step result: a JSON object with outcome and summary";
/// Claude Code's tools that change the file named in their input.
const EDIT_TOOLS: [&str; 4] = ["Edit", "MultiEdit", "Write", "NotebookEdit"];

/// Follows one invocation's output: tool calls wait for their results, and usage is counted once.
#[derive(Clone, Debug, Default)]
pub struct ClaudeStream {
  /// Tool calls without a result yet, by tool use id.
  calls: HashMap<String, Call>,
  /// The most usage each assistant message reported so far, part by part, by message id.
  messages: HashMap<String, TokenUsage>,
  /// Everything counted so far.
  counted: TokenUsage,
  /// The last text the agent wrote.
  last_text: Option<String>,
  signed_out: bool,
  /// The MCP tools that run without asking, in a step that asks before MCP actions.
  exemptions: InvocationExemptions,
  /// What the CLI printed back of its input since it was last taken, oldest first.
  replayed: Vec<String>,
  /// Results seen: each ends a turn.
  results: usize,
  /// The CLI printed back a user message: it took the prompt.
  prompt_taken: bool,
}

#[derive(Clone, Debug)]
struct Call {
  tool: String,
  command: String,
  /// The file an edit tool changes.
  file: Option<String>,
  /// The file Orchester's edit tool changes, reported after the call's end.
  changes: Option<FileChange>,
}

impl ClaudeStream {
  /// A stream whose calls of the tools `exemptions` names are reported as not asked
  /// ([`AgentEvent::McpToolNotAsked`]): they run by an allow rule, never reaching the permission prompt.
  pub fn with_exemptions(exemptions: InvocationExemptions) -> Self {
    Self {
      exemptions,
      ..Self::default()
    }
  }

  /// The events one line of Claude Code output reports, possibly none: a repeated usage, an edit
  /// whose result is still to come or the end of a CLI that already said it is not signed in.
  /// Standard error carries the CLI's own diagnostics, which are kept as raw lines, like anything
  /// else the parser does not understand.
  pub fn parse(&mut self, stream: OutputStream, line: &str) -> Vec<AgentEvent> {
    let raw = || AgentEvent::Raw {
      stream,
      line: line.to_owned(),
    };
    if stream == OutputStream::Stderr {
      return vec![raw()];
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
      return vec![raw()];
    };
    let events = match value.get("type").and_then(Value::as_str) {
      // Account limits never reach the transcript; one this version cannot read is left out.
      Some("rate_limit_event") => Some(
        value
          .get("rate_limit_info")
          .and_then(super::super::plan_limits::claude_limits)
          .map(|limits| vec![AgentEvent::PlanLimits { limits }])
          .unwrap_or_default(),
      ),
      Some("system") => system(&value),
      Some("assistant") => self.assistant(&value).map(|(mut events, understood)| {
        if !understood {
          events.push(raw());
        }
        events
      }),
      Some("user") => self.user(&value),
      Some("result") => self.result(&value),
      _ => None,
    };
    events.unwrap_or_else(|| vec![raw()])
  }

  /// The user messages the CLI took from its input since the last call, as it printed them back.
  pub fn take_replayed(&mut self) -> Vec<String> {
    std::mem::take(&mut self.replayed)
  }

  /// How many results the CLI printed: how many turns it ended.
  pub const fn results(&self) -> usize {
    self.results
  }

  /// The message's events, and whether every content block was understood.
  fn assistant(&mut self, value: &Value) -> Option<(Vec<AgentEvent>, bool)> {
    let message = value.get("message")?;
    let id = text(message, "id")?;
    let error = value.get("error").and_then(Value::as_str);
    let mut events = Vec::new();
    if !self.messages.contains_key(&id) {
      events.push(AgentEvent::TurnStarted);
    }
    let mut understood = true;
    for block in message.get("content")?.as_array()? {
      let kind = block.get("type").and_then(Value::as_str).unwrap_or_default();
      understood &= KNOWN_BLOCKS.contains(&kind);
      if let Some(event) = self.content(block, error) {
        events.push(event);
      }
      if kind == "tool_use" {
        events.extend(orchester_call(block));
        events.extend(self.not_asked(block));
      }
    }
    let usage = message.get("usage").and_then(usage).unwrap_or_default();
    let mut seen = self.messages.get(&id).copied().unwrap_or_default();
    if let Some(grown) = growth(usage, seen) {
      seen += grown;
      self.counted += grown;
      events.push(AgentEvent::TurnCompleted { usage: grown });
    }
    self.messages.insert(id, seen);
    Some((events, understood))
  }

  fn content(&mut self, block: &Value, error: Option<&str>) -> Option<AgentEvent> {
    match block.get("type")?.as_str()? {
      "text" => {
        let text = text(block, "text")?;
        Some(match error {
          Some(AUTHENTICATION_FAILED) => {
            self.signed_out = true;
            AgentEvent::NotSignedIn { message: text }
          }
          Some(_) => AgentEvent::Error { message: text },
          None => {
            self.last_text = Some(text.clone());
            AgentEvent::Message { text }
          }
        })
      }
      "thinking" => Some(AgentEvent::Reasoning {
        text: text(block, "thinking").filter(|thinking| !thinking.trim().is_empty())?,
      }),
      "tool_use" => self.tool_use(block),
      _ => None,
    }
  }

  /// A tool call: commands and other tools start a command entry; an edit is reported once its
  /// result says it happened.
  fn tool_use(&mut self, block: &Value) -> Option<AgentEvent> {
    let item = text(block, "id")?;
    let tool = text(block, "name")?;
    let input = block.get("input").unwrap_or(&Value::Null);
    let command = describe(&tool, input);
    let file = EDIT_TOOLS.contains(&tool.as_str()).then(|| file_path(input)).flatten();
    let changes = OrchesterTool::from_claude_name(&tool).and_then(|own| own.changed_file(input));
    let started = file.is_none().then(|| AgentEvent::CommandStarted {
      item: item.clone(),
      command: command.clone(),
    });
    self.calls.insert(
      item,
      Call {
        tool,
        command,
        file,
        changes,
      },
    );
    started
  }

  /// A call of a tool that runs without asking.
  fn not_asked(&self, block: &Value) -> Option<AgentEvent> {
    let (server, tool) = self.exemptions.claude_tool(block.get("name")?.as_str()?)?;
    Some(AgentEvent::McpToolNotAsked {
      item: text(block, "id")?,
      server: server.to_string(),
      tool: tool.to_string(),
    })
  }

  /// Tool results, or input the CLI printed back; a user message without either is not understood.
  fn user(&mut self, value: &Value) -> Option<Vec<AgentEvent>> {
    if value.get("isReplay").and_then(Value::as_bool) == Some(true) {
      self.prompt_taken = true;
      self.replayed.push(result_text(value.get("message")?.get("content")));
      return Some(Vec::new());
    }
    let results: Vec<&Value> = value
      .get("message")?
      .get("content")?
      .as_array()?
      .iter()
      .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
      .collect();
    if results.is_empty() {
      return None;
    }
    let created = value
      .get("tool_use_result")
      .and_then(|result| result.get("type"))
      .and_then(Value::as_str)
      == Some("create");
    let events: Option<Vec<Vec<AgentEvent>>> = results
      .into_iter()
      .map(|block| self.tool_result(block, created))
      .collect();
    events.map(|events| events.into_iter().flatten().collect())
  }

  /// The end of a tool call: the change an edit made, or the end of its command, followed by the change
  /// of Orchester's edit tool when it succeeded.
  fn tool_result(&mut self, block: &Value, created: bool) -> Option<Vec<AgentEvent>> {
    let item = text(block, "tool_use_id")?;
    let call = self.calls.remove(&item)?;
    let failed = block.get("is_error").and_then(Value::as_bool).unwrap_or(false);
    if let (Some(path), false) = (call.file, failed) {
      let kind = if call.tool == "Write" && created {
        FileChangeKind::Added
      } else {
        FileChangeKind::Modified
      };
      return Some(vec![AgentEvent::FileChanged {
        item,
        changes: vec![FileChange { path, kind }],
      }]);
    }
    let changed = call.changes.filter(|_| !failed).map(|change| AgentEvent::FileChanged {
      item: item.clone(),
      changes: vec![change],
    });
    let finished = AgentEvent::CommandFinished {
      item,
      command: call.command,
      output: result_text(block.get("content")),
      exit_code: None,
      status: if failed {
        CommandStatus::Failed
      } else {
        CommandStatus::Completed
      },
    };
    Some(std::iter::once(finished).chain(changed).collect())
  }

  fn result(&mut self, value: &Value) -> Option<Vec<AgentEvent>> {
    let is_error = value.get("is_error")?.as_bool()?;
    let said = text(value, "result").filter(|result| !result.trim().is_empty());
    let own_turn = !is_error && !self.prompt_taken && said.is_none() && value.get("structured_output").is_none();
    let mut events = Vec::new();
    let total = value.get("usage").and_then(usage).unwrap_or_default();
    if let Some(grown) = growth(total, self.counted) {
      self.counted += grown;
      events.push(AgentEvent::TurnCompleted { usage: grown });
    }
    for denial in value
      .get("permission_denials")
      .and_then(Value::as_array)
      .into_iter()
      .flatten()
    {
      let (Some(item), Some(tool)) = (text(denial, "tool_use_id"), text(denial, "tool_name")) else {
        continue;
      };
      let command = describe(&tool, denial.get("tool_input").unwrap_or(&Value::Null));
      events.push(AgentEvent::PermissionDenied { item, command });
    }
    if own_turn {
      return Some(events);
    }
    self.results += 1;
    if is_error {
      // A CLI that is not signed in already said so.
      if !self.signed_out {
        let message = said.or_else(|| text(value, "subtype")).unwrap_or_default();
        events.push(AgentEvent::TurnFailed { message });
      }
    } else {
      if let Some(said) = said.filter(|said| self.last_text.as_ref() != Some(said)) {
        self.last_text = Some(said.clone());
        events.push(AgentEvent::Message { text: said });
      }
      events.extend(self.step_result(value));
    }
    Some(events)
  }

  /// The step result as the last message, unless the last message already is exactly it; a
  /// failed turn without one.
  fn step_result(&mut self, value: &Value) -> Option<AgentEvent> {
    let structured = value
      .get("structured_output")
      .and_then(|output| serde_json::from_value::<StepResult>(output.clone()).ok());
    let Some(result) = structured.or_else(|| self.last_text.as_deref().and_then(StepResult::at_end)) else {
      return Some(AgentEvent::TurnFailed {
        message: NO_RESULT.to_owned(),
      });
    };
    if self.last_text.as_deref().and_then(StepResult::parse).as_ref() == Some(&result) {
      return None;
    }
    let text = serde_json::to_string(&result).expect("a step result serializes to JSON");
    self.last_text = Some(text.clone());
    Some(AgentEvent::Message { text })
  }
}

/// The session, and every MCP server the CLI could not start: `init` lists each with its status,
/// `connected` for one that started (probe 2026-09-28: `failed` for a missing program, a server that
/// exits and an unreachable URL).
fn system(value: &Value) -> Option<Vec<AgentEvent>> {
  match value.get("subtype")?.as_str()? {
    "init" => {
      let mut events = vec![AgentEvent::SessionStarted {
        session: text(value, "session_id")?,
      }];
      let servers = value.get("mcp_servers").and_then(Value::as_array);
      events.extend(servers.into_iter().flatten().filter_map(failed_server));
      Some(events)
    }
    _ => None,
  }
}

fn failed_server(server: &Value) -> Option<AgentEvent> {
  let status = server.get("status")?.as_str()?;
  if matches!(status, "connected" | "pending") {
    return None;
  }
  Some(AgentEvent::McpServer {
    server: Some(text(server, "name")?),
    problem: McpServerProblem::FailedToStart {
      detail: (status != "failed").then(|| status.to_owned()),
    },
  })
}

/// What a call of one of Orchester's file tools asked, told right after its start so that its row reads as the native
/// agent's call of the tool.
fn orchester_call(block: &Value) -> Option<AgentEvent> {
  let tool = OrchesterTool::from_claude_name(block.get("name")?.as_str()?)?;
  Some(AgentEvent::NativeCall {
    item: text(block, "id")?,
    call: tool.shown_call(block.get("input").unwrap_or(&Value::Null))?,
  })
}

/// A tool call as one line: a shell command as it is, another tool with what it works on.
fn describe(tool: &str, input: &Value) -> String {
  let subject = match tool {
    "Bash" => return text(input, "command").unwrap_or_else(|| tool.to_owned()),
    "Glob" | "Grep" => text(input, "pattern"),
    "WebFetch" => text(input, "url"),
    "WebSearch" => text(input, "query"),
    _ => file_path(input),
  };
  match subject {
    Some(subject) => format!("{tool} {subject}"),
    None => tool.to_owned(),
  }
}

fn file_path(input: &Value) -> Option<String> {
  ["file_path", "notebook_path", "path", "filename", "base_directory"]
    .into_iter()
    .find_map(|name| text(input, name))
}

/// A tool result's content: text, or the text parts of a list of blocks.
fn result_text(content: Option<&Value>) -> String {
  match content {
    Some(Value::String(text)) => text.clone(),
    Some(Value::Array(parts)) => parts
      .iter()
      .filter_map(|part| part.get("text").and_then(Value::as_str))
      .collect::<Vec<_>>()
      .join("\n"),
    _ => String::new(),
  }
}

/// A `usage` object of Claude Code's output: cache reads are cached input, cache writes count as input.
pub(crate) fn usage(usage: &Value) -> Option<TokenUsage> {
  let count = |name| usage.get(name).and_then(Value::as_u64).unwrap_or(0);
  let input = usage.get("input_tokens")?.as_u64()?;
  let cache_read = count("cache_read_input_tokens");
  Some(TokenUsage {
    input: input
      .saturating_add(count("cache_creation_input_tokens"))
      .saturating_add(cache_read),
    cached_input: cache_read,
    output: usage.get("output_tokens")?.as_u64()?,
  })
}

/// What `now` holds beyond `before`, part by part; `None` when nothing grew.
fn growth(now: TokenUsage, before: TokenUsage) -> Option<TokenUsage> {
  let grown = TokenUsage {
    input: now.input.saturating_sub(before.input),
    cached_input: now.cached_input.saturating_sub(before.cached_input),
    output: now.output.saturating_sub(before.output),
  };
  (grown != TokenUsage::default()).then_some(grown)
}

fn text(value: &Value, name: &str) -> Option<String> {
  value.get(name)?.as_str().map(str::to_owned)
}
