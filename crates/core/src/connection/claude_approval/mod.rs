//! Interactive approvals for Claude Code, the counterpart of Codex App Server's approval requests
//! (`docs/interactive-tool-approvals.md`).
//!
//! `claude -p --permission-prompt-tool <mcp tool>` ("MCP tool to use for permission prompts (only
//! works with --print)", Claude Code 2.1.89; hidden from `--help`) calls that tool whenever its own
//! permission check would ask a person, with `{tool_name, input, tool_use_id}`, and takes the text
//! it returns as the decision: `{"behavior":"allow","updatedInput":{…}}` or
//! `{"behavior":"deny","message":"…"}`. What the check allows or denies by itself never reaches the
//! tool, and the CLI removes the tool from those the model may call. The CLI waits for the call as
//! for any MCP tool call (`MCP_TOOL_TIMEOUT`, about 28 hours by default).
//!
//! Orchester serves that tool for each invocation that asks a person: an MCP server named
//! [`SERVER`] (a capital first letter, so no project server can take the name), started by Claude
//! Code from the invocation's own configuration file ([`ClaudeApprovalConfig`]). The server is a
//! small relay ([`relay`]) that hands each call over to the runner on a loopback connection opened
//! with the invocation's random [`ApprovalToken`]; the runner shows the request, waits for one
//! person's decision and answers exactly that call.
//!
//! [`ClaudeApprovalScope`] decides what may reach a person: a `Bash` command (one outside the allowed
//! list, or every command when the step asks before commands or lists none; also in a read-only step),
//! and a tool of one of the invocation's MCP servers when it asks before MCP actions. Anything else
//! the CLI would ask about (an edit or a read outside the working folders, a web fetch) is denied at
//! once, as print mode denies it without the tool, so a prompt never grants more than the step's
//! permissions leave to a person.
//!
//! The same server serves the step's Orchester tools (`crate::orchester_tools`): those that run have
//! `mcp__Orchester__<tool>` allow rules, and a call of one that asks reaches the prompt and a person
//! whether or not the step asks before MCP actions. Claude Code 2.1.286 removes only the prompt tool
//! itself from the tools the model may call (`permissionPromptToolName` filters by exact name), so the
//! server's other tools stay offered.
//!
//! A decision covers one call: an approval answers `{"behavior":"allow","updatedInput":{}}`, which
//! 2.1.89 runs with the call's own input (an empty `updatedInput` keeps it), and never returns
//! `updatedPermissions`, the rules that would allow later calls. The answer never repeats the input,
//! so no value the agent put in it comes back through the tool.

pub mod relay;

use std::{collections::BTreeMap, fmt, sync::Arc};

use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
  command::{SecretVariable, redact_secrets},
  connection::mcp::McpServerName,
  orchester_tools::StepTools,
  permission::{AgentPermissions, CommandPermission},
  run::recorded_arguments,
  secret::SecretValue,
};

/// The MCP server that serves the permission prompt.
pub const SERVER: &str = "Orchester";
/// Its one tool.
pub const TOOL: &str = "approve";
/// The tool as `--permission-prompt-tool` names it.
pub const PROMPT_TOOL: &str = "mcp__Orchester__approve";
/// The relay's variable holding the runner's loopback address (`127.0.0.1:<port>`).
pub const ADDRESS_VARIABLE: &str = "ORCHESTER_APPROVAL_ADDRESS";
/// The relay's variable holding the invocation's [`ApprovalToken`].
pub const TOKEN_VARIABLE: &str = "ORCHESTER_APPROVAL_TOKEN";
/// How Claude Code names the tools of [`SERVER`]; no project server has a capital letter in its name.
const ORCHESTER_TOOLS: &str = "mcp__Orchester__";
/// The only command tool of Claude Code.
const COMMAND_TOOL: &str = "Bash";
/// What an approval's server is called when it is a command, as for Codex.
const COMMAND_SERVER: &str = "command";
const DENIED: &str = "Denied in Orchester.";
const UNAVAILABLE: &str = "Orchester approvals are unavailable; nothing was approved.";
const NOT_ASKABLE: &str = "This step's permissions do not allow this action, and it cannot be approved in Orchester.";

/// The secret that lets only the relay of one invocation reach its runner: 244 random bits.
/// `Debug` shows nothing of it, and it has no `Display`.
pub struct ApprovalToken(Zeroizing<String>);

impl ApprovalToken {
  pub fn generate() -> Self {
    Self(Zeroizing::new(format!(
      "{}{}",
      Uuid::new_v4().simple(),
      Uuid::new_v4().simple()
    )))
  }

  /// Whether `text` is this token, compared in constant time for texts of its length.
  pub fn matches(&self, text: &str) -> bool {
    let (token, text) = (self.0.as_bytes(), text.as_bytes());
    token.len() == text.len() && token.iter().zip(text).fold(0, |differ, (a, b)| differ | (a ^ b)) == 0
  }

  /// The value, for the configuration file the relay starts with.
  pub fn expose(&self) -> &str {
    &self.0
  }

  /// The token as the relay's variable [`TOKEN_VARIABLE`], for a CLI that hands its servers variables
  /// from its own environment (Codex) instead of a configuration file.
  pub fn variable(&self) -> SecretVariable {
    let value = SecretValue::new(self.0.to_string()).expect("a generated token is never blank");
    SecretVariable::new(TOKEN_VARIABLE, Arc::new(value))
  }
}

impl fmt::Debug for ApprovalToken {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("ApprovalToken(<redacted>)")
  }
}

/// The `--mcp-config` file that starts the relay: `program` with `args`, the runner's `address`
/// and the token in its environment. `Debug` hides the contents, which are wiped when dropped.
pub struct ClaudeApprovalConfig {
  contents: Zeroizing<String>,
}

impl ClaudeApprovalConfig {
  pub fn new(program: &str, args: &[&str], address: &str, token: &ApprovalToken) -> Self {
    // Serialized from borrowed strings, so the token is copied only into the wiped contents.
    #[derive(Serialize)]
    struct Server<'a> {
      r#type: &'static str,
      command: &'a str,
      args: &'a [&'a str],
      env: BTreeMap<&'static str, &'a str>,
    }
    let server = Server {
      r#type: "stdio",
      command: program,
      args,
      env: BTreeMap::from([(ADDRESS_VARIABLE, address), (TOKEN_VARIABLE, token.expose())]),
    };
    let file = BTreeMap::from([("mcpServers", BTreeMap::from([(SERVER, server)]))]);
    let contents = serde_json::to_string(&file).expect("strings and maps of strings serialize to JSON");
    Self {
      contents: Zeroizing::new(contents),
    }
  }

  pub fn contents(&self) -> &str {
    &self.contents
  }
}

impl fmt::Debug for ClaudeApprovalConfig {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("ClaudeApprovalConfig(<redacted>)")
  }
}

/// What one Claude Code invocation may leave to a person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaudeApprovalScope {
  commands: bool,
  tools: bool,
  servers: Vec<McpServerName>,
  /// Secret values of those servers, removed from the arguments an approval shows.
  secrets: Vec<SecretVariable>,
  /// The step's Orchester tools; a call of one that asks reaches a person.
  orchester: StepTools,
}

impl ClaudeApprovalScope {
  /// The scope of an invocation with the enforced `permissions` and the MCP servers `servers`; `None`
  /// when it asks a person for nothing and so needs no permission prompt. Commands ask whatever the
  /// repository access (`crate::permission::claude`): every one when the step asks before commands or
  /// lists none, those outside the list otherwise. In Autopilot every command and every call of the
  /// servers' tools reaches Orchester (`crate::permission::EffectiveApprovalMode`).
  pub fn of(permissions: &AgentPermissions, servers: &[McpServerName]) -> Option<Self> {
    let autopilot = permissions.hands_requests_to_orchester();
    let commands = autopilot || *permissions.commands() != CommandPermission::AnyInWorktree;
    let tools = autopilot || permissions.interactive_tool_approvals();
    (commands || tools).then(|| Self {
      commands,
      tools,
      servers: servers.to_vec(),
      secrets: Vec::new(),
      orchester: StepTools::default(),
    })
  }

  /// This scope, leaving the calls of `tools` that ask to a person.
  #[must_use]
  pub fn with_orchester_tools(mut self, tools: StepTools) -> Self {
    self.orchester = tools;
    self
  }

  /// This scope, removing `secrets` from the MCP arguments its requests show.
  #[must_use]
  pub fn with_secrets(mut self, secrets: Vec<SecretVariable>) -> Self {
    self.secrets = secrets;
    self
  }

  /// What to do with the permission prompt call whose arguments are `arguments`.
  pub fn classify(&self, arguments: &Value) -> ClaudeAsk {
    let (Some(tool), Some(call)) = (
      arguments.get("tool_name").and_then(Value::as_str),
      arguments
        .get("tool_use_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty()),
    ) else {
      return ClaudeAsk::Refused;
    };
    let input = arguments.get("input").unwrap_or(&Value::Null);
    let text = |key: &str| input.get(key).and_then(Value::as_str).map(str::to_owned);
    if tool == COMMAND_TOOL {
      let (true, Some(command)) = (self.commands, text("command")) else {
        return ClaudeAsk::Refused;
      };
      return ClaudeAsk::Person(ClaudeApprovalRequest {
        call: call.to_owned(),
        command: true,
        server: COMMAND_SERVER.to_owned(),
        action: command,
        target: None,
        scope: text("description"),
      });
    }
    if let Some(name) = tool.strip_prefix(ORCHESTER_TOOLS) {
      if !self.orchester.asks(name) {
        return ClaudeAsk::Refused;
      }
      return ClaudeAsk::Person(ClaudeApprovalRequest {
        call: call.to_owned(),
        command: false,
        server: SERVER.to_owned(),
        action: name.to_owned(),
        target: None,
        scope: Some(shown_arguments(input, &self.secrets)),
      });
    }
    let Some((server, action)) = self.server_tool(tool).filter(|_| self.tools) else {
      return ClaudeAsk::Refused;
    };
    ClaudeAsk::Person(ClaudeApprovalRequest {
      call: call.to_owned(),
      command: false,
      server: server.to_string(),
      action: action.to_owned(),
      target: text("filePath"),
      scope: Some(shown_arguments(input, &self.secrets)),
    })
  }

  /// The invocation's server and the tool of `tool`, named `mcp__<server>__<tool>`; the longest
  /// server name wins, since names may contain `__`.
  fn server_tool<'a>(&self, tool: &'a str) -> Option<(&McpServerName, &'a str)> {
    let rest = tool.strip_prefix("mcp__")?;
    self
      .servers
      .iter()
      .filter_map(|server| {
        let action = rest.strip_prefix(server.as_str())?.strip_prefix("__")?;
        (!action.is_empty()).then_some((server, action))
      })
      .max_by_key(|(server, _)| server.as_str().len())
  }
}

/// Where a permission prompt call goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaudeAsk {
  /// A person decides.
  Person(ClaudeApprovalRequest),
  /// The step's permissions do not leave it to a person: denied at once ([`refusal`]).
  Refused,
}

/// A call waiting for a person, with what the approval shows. `Debug` leaves out the action and
/// the target, which carry what the agent wrote.
#[derive(Clone, PartialEq, Eq)]
pub struct ClaudeApprovalRequest {
  call: String,
  command: bool,
  server: String,
  action: String,
  target: Option<String>,
  scope: Option<String>,
}

impl ClaudeApprovalRequest {
  /// A call `call` of Orchester's tool `tool` with `arguments` that reached the runner without an
  /// approval, which the runner asks a person about before it runs it; `secrets` are removed from the
  /// arguments the approval shows.
  pub fn tool_call(call: String, tool: &str, arguments: &Value, secrets: &[SecretVariable]) -> Self {
    Self {
      call,
      command: false,
      server: SERVER.to_owned(),
      action: tool.to_owned(),
      target: None,
      scope: Some(shown_arguments(arguments, secrets)),
    }
  }

  /// This request, showing `target` as what the call works on: the folder a call of Orchester's tools outside the
  /// worktree reads in, which Approve similar keeps the rule to.
  #[must_use]
  pub fn with_target(mut self, target: String) -> Self {
    self.target = Some(target);
    self
  }

  /// The tool use the CLI asks about, unique within its session.
  pub fn call(&self) -> &str {
    &self.call
  }

  pub const fn is_command(&self) -> bool {
    self.command
  }

  /// `command` for a command, else the MCP server.
  pub fn server(&self) -> &str {
    &self.server
  }

  /// The command, or the MCP tool.
  pub fn action(&self) -> &str {
    &self.action
  }

  /// The file an MCP tool names as `filePath`, or the folder a call of Orchester's tools outside the worktree reads
  /// in; the runner shows the worktree for a command.
  pub fn target(&self) -> Option<&str> {
    self.target.as_deref()
  }

  /// What the agent says the command is for, or an MCP tool's arguments ([`shown_arguments`]).
  pub fn scope(&self) -> Option<&str> {
    self.scope.as_deref()
  }
}

/// An MCP call's arguments as the approval shows them: every text in them with `secrets` removed,
/// then JSON on one line, recorded whole ([`recorded_arguments`] cuts only a pathological call).
/// Removing them from the texts first means a value that JSON escapes (quotes, backslashes) or that
/// a cut would split is still found whole.
fn shown_arguments(input: &Value, secrets: &[SecretVariable]) -> String {
  recorded_arguments(redacted(input, secrets).to_string())
}

impl fmt::Debug for ClaudeApprovalRequest {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ClaudeApprovalRequest")
      .field("call", &self.call)
      .field("server", &self.server)
      .finish_non_exhaustive()
  }
}

/// The permission prompt's answer on one call: allowed, or denied with `denial`, which Claude Code
/// hands to the agent as the tool's result; the agent then goes on without the call.
pub fn prompt_result(denial: Option<&str>) -> String {
  match denial {
    None => json!({"behavior": "allow", "updatedInput": {}}).to_string(),
    Some(message) => json!({"behavior": "deny", "message": message}).to_string(),
  }
}

/// The permission prompt's answer to a call nobody answered: the worker ended, or the call was asked
/// about before.
pub fn not_answered() -> String {
  prompt_result(Some(DENIED))
}

/// `value` with `secrets` removed from every text in it, keys included. A number whose digits hold a
/// secret, such as a numeric PIN, becomes the redacted text in its place.
fn redacted(value: &Value, secrets: &[SecretVariable]) -> Value {
  match value {
    Value::String(text) => Value::String(redact_secrets(text, secrets)),
    Value::Number(number) => {
      let digits = number.to_string();
      let text = redact_secrets(&digits, secrets);
      if text == digits {
        value.clone()
      } else {
        Value::String(text)
      }
    }
    Value::Array(items) => Value::Array(items.iter().map(|item| redacted(item, secrets)).collect()),
    Value::Object(fields) => Value::Object(
      fields
        .iter()
        .map(|(key, field)| (redact_secrets(key, secrets), redacted(field, secrets)))
        .collect(),
    ),
    other => other.clone(),
  }
}

/// The answer to a call the relay could not hand to Orchester: denied, and not by a person.
pub fn unavailable() -> String {
  json!({"behavior": "deny", "message": UNAVAILABLE}).to_string()
}

/// The answer to a call no person may decide.
pub fn refusal() -> String {
  json!({"behavior": "deny", "message": NOT_ASKABLE}).to_string()
}
