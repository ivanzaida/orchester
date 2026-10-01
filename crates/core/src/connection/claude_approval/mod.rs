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
//! [`ClaudeApprovalScope`] decides what may reach a person: a `Bash` command when the step asks
//! before commands or has allowed commands (one outside the list asks, also in a read-only step),
//! and a tool of one of the invocation's MCP servers when it asks before MCP actions. Anything else
//! the CLI would ask about (an edit or a read outside the working folders, a web fetch) is denied at
//! once, as print mode denies it without the tool, so a prompt never grants more than the step's
//! permissions leave to a person.
//!
//! A decision covers one call: an approval answers `{"behavior":"allow","updatedInput":{}}`, which
//! 2.1.89 runs with the call's own input (an empty `updatedInput` keeps it), and never returns
//! `updatedPermissions`, the rules that would allow later calls. The answer never repeats the input,
//! so no value the agent put in it comes back through the tool.

pub mod relay;

use std::{collections::BTreeMap, fmt};

use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
  command::{SecretVariable, redact_secrets},
  connection::mcp::McpServerName,
  permission::{AgentPermissions, CommandPermission, RepositoryAccess},
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
/// The only command tool of Claude Code.
const COMMAND_TOOL: &str = "Bash";
/// What an approval's server is called when it is a command, as for Codex.
const COMMAND_SERVER: &str = "command";
const DENIED: &str = "Denied in Orchester.";
/// How much of an MCP call's arguments an approval shows.
const SHOWN_ARGUMENTS: usize = 600;
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
}

impl ClaudeApprovalScope {
  /// The scope of an invocation with the enforced `permissions` and the MCP servers `servers`; `None`
  /// when it asks a person for nothing and so needs no permission prompt. Commands ask when the step
  /// asks before each one (a read-only step then gets no `Bash`, `crate::permission::claude`) or when
  /// it has allowed commands, for those outside the list.
  pub fn of(permissions: &AgentPermissions, servers: &[McpServerName]) -> Option<Self> {
    let commands = match permissions.commands() {
      CommandPermission::AskBefore => permissions.repository() == RepositoryAccess::Write,
      CommandPermission::Allowed(list) => !list.is_empty(),
      CommandPermission::AnyInWorktree => false,
    };
    let tools = permissions.interactive_tool_approvals();
    (commands || tools).then(|| Self {
      commands,
      tools,
      servers: servers.to_vec(),
      secrets: Vec::new(),
    })
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

  /// The file an MCP tool names as `filePath`; the runner shows the worktree for a command.
  pub fn target(&self) -> Option<&str> {
    self.target.as_deref()
  }

  /// What the agent says the command is for, or an MCP tool's arguments ([`shown_arguments`]).
  pub fn scope(&self) -> Option<&str> {
    self.scope.as_deref()
  }
}

/// An MCP call's arguments as the approval shows them: every text in them with `secrets` removed,
/// then JSON on one line, then cut after [`SHOWN_ARGUMENTS`] characters. Removing them from the
/// texts first means a value that JSON escapes (quotes, backslashes) or that the cut would split
/// is still found whole.
fn shown_arguments(input: &Value, secrets: &[SecretVariable]) -> String {
  let text = redacted(input, secrets).to_string();
  match text.char_indices().nth(SHOWN_ARGUMENTS) {
    Some((end, _)) => format!("{}…", &text[..end]),
    None => text,
  }
}

impl fmt::Debug for ClaudeApprovalRequest {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ClaudeApprovalRequest")
      .field("call", &self.call)
      .field("server", &self.server)
      .finish_non_exhaustive()
  }
}

/// The permission prompt's answer to a person's decision on one call.
pub fn prompt_result(approve: bool) -> String {
  if approve {
    json!({"behavior": "allow", "updatedInput": {}}).to_string()
  } else {
    json!({"behavior": "deny", "message": DENIED}).to_string()
  }
}

/// `value` with `secrets` removed from every text in it, keys included.
fn redacted(value: &Value, secrets: &[SecretVariable]) -> Value {
  match value {
    Value::String(text) => Value::String(redact_secrets(text, secrets)),
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
