//! The Orchestrator chat's MCP server tools (ORC-89, `docs/design/orchestrator-toolset.md`, decision 10): adding a
//! server to the chat's project, changing one, turning one on or off and removing one, through the same fields, rules
//! and refusals as Orchester's own `add_mcp_server`, `update_mcp_server`, `set_mcp_server_enabled` and
//! `remove_mcp_server` (Connections › MCP servers). Each call is read first as the tool would make it
//! ([`crate::app_mcp::setup::preview_mcp`]): what widens what agents may do (a new server, another program, endpoint,
//! authorization, variable or exempt tool, turning one on) is always a card only a person applies, in any turn and
//! whatever the switches say; a change that only narrows (turning one off, removing one no profile chooses, dropping
//! exempt tools or the authorization) is made at once only in a turn the person asked for and while they let
//! automation change agent permissions, and is a card otherwise. A person's *Apply* reads it again and refuses it once
//! anything it shows or stands on changed; the widening they were shown is theirs to decide, as in the window. A
//! secret is only ever named (`NAME@environment`): no tool takes or shows its value.

use orchester_core::orchestrator::{
  ActionOutcome, ChatAction,
  change::{Change, ProposedChange},
  mcp::{McpChange, McpOperation},
};
use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool, ToolAnnotations},
};
use serde_json::{Value, json};

use super::{
  change, keep,
  own_project::in_own_project,
  tools::{ChatScope, app_tool, changes_permissions},
};
use crate::{
  app_mcp::{
    PersonDecision, Services, ToolError, Translate, blocking,
    setup::{AddMcpServer, InServer, SetMcpServerEnabled, UpdateMcpServer, preview_mcp},
  },
  messages::Message,
  runner::Runners,
};

/// The chat's MCP server tools, by name.
pub const NAMES: [&str; 4] = [
  "mcp_add_server",
  "mcp_update_server",
  "mcp_set_server_enabled",
  "mcp_remove_server",
];

/// What the chat's agent reads of every tool: where a card goes and what a secret is.
const RULES: &str = "Adding a server, or a change that widens what agents may do (another name, program, arguments, \
  folder or endpoint, another authorization that sends a secret, any variable but RUST_LOG, RUST_BACKTRACE, CI, \
  NO_COLOR, FORCE_COLOR, TZ, LANG and the LC_ ones, a variable that takes a secret, a tool added to exempt_tools, \
  turning a server on), is always a card in this conversation that the person applies or dismisses, in any turn and \
  whatever the person's switches say: nothing changes until they apply it, and you never apply it. The card shows the \
  server as applying leaves it: the program and its arguments or the URL, the authorization's scheme and which secret, \
  the variables' names, which tool calls run without asking, and a warning when the program is not found on this \
  computer's search path. A change that only narrows (turning a server off, removing one no profile chooses, dropping \
  exempt tools or the authorization) is made at once only while the person turned on Let automation change agent \
  permissions and they asked for it in this turn; otherwise it is a card too. Each answer says which happened. A \
  secret is never a value: name an active project secret NAME@environment (list_secrets), in authorization or as \
  ${secret:NAME@environment} in a variable's value; never type or ask for a secret's value. Refused as the app's own \
  tool refuses it (invalid_server, name_taken, field_not_applicable, program_in_worktree, program_in_repository, \
  program_not_local, in_use, not_found), and when applied, changed_since_proposed once the server, a secret it names, \
  the program's file or the profiles that choose it changed since it was proposed.";

/// The chat's MCP server tools as its `tools/list` answers.
pub fn tools() -> Vec<Tool> {
  let tool = |name: &'static str, does: &str, schema| {
    Tool::new(name, format!("{does} {RULES}"), schema).with_annotations(
      ToolAnnotations::new()
        .read_only(false)
        .destructive(name == NAMES[3])
        .open_world(false),
    )
  };
  vec![
    tool(
      NAMES[0],
      "Proposes adding an MCP server to this project, as Connections › Add MCP server does: a unique `name` (small \
       letters, digits, - and _, starting with a letter) and either a `program` the agent's CLI starts with each agent \
       (a name on the search path or an absolute path, stored as the fixed file it is, outside the project's \
       repository; with `args`, `working_directory` and `environment`) or a remote `url` (with an optional \
       `authorization`: scheme bearer, basic with a username, basic_credential or header with a header name, each \
       with its secret), and `exempt_tools`, the server's tools that run without asking for profiles that ask before \
       MCP actions. Suggest a program only from what you read in the project or a tool's own documentation, and tell \
       the person where it came from; never guess a path. Once added, a server runs only for profiles that choose it \
       (save_profile_draft).",
      schema_for_type::<AddMcpServer>(),
    ),
    tool(
      NAMES[1],
      "Proposes changing an MCP server's fields, as Edit MCP server does; a field left out keeps its value, and \
       `program` or `url` switches how it is reached.",
      schema_for_type::<UpdateMcpServer>(),
    ),
    tool(
      NAMES[2],
      "Turns an MCP server on or off for every profile that chooses it, as its inspector's Turn on and Turn off do: \
       agents that start from then on get it, or work without it.",
      schema_for_type::<SetMcpServerEnabled>(),
    ),
    tool(
      NAMES[3],
      "Removes an MCP server no agent profile chooses (in_use names the profiles that do: they switch first, or turn \
       the server off, which keeps its settings).",
      schema_for_type::<InServer>(),
    ),
  ]
}

/// Whether `name` is one of the chat's MCP server tools.
pub fn is_tool(name: &str) -> bool {
  NAMES.contains(&name)
}

/// Calls the chat's MCP server tool `name` with `arguments` in `scope`: a card, or a narrowing made at once; kept in
/// the conversation either way, refused or not, and `runners` tell the chat it changed.
pub(super) async fn call(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  name: &str,
  mut arguments: JsonObject,
) -> Result<Value, ToolError> {
  let called = match in_own_project(scope, &mut arguments) {
    Ok(()) => act((services, translate), scope, name, arguments).await,
    Err(error) => Err(error),
  };
  let (result, change, outcome) = match called {
    Ok(Acted::Proposed(change)) => (Ok(change::told(translate, &change)), Some(change), ActionOutcome::Waits),
    Ok(Acted::Made(result, change)) => {
      let outcome = match &result {
        Ok(_) => ActionOutcome::Done,
        Err(error) => keep::refusal(error, translate),
      };
      (result, Some(change), outcome)
    }
    Err(error) => {
      let outcome = keep::refusal(&error, translate);
      (Err(error), None, outcome)
    }
  };
  let action = ChatAction {
    change: change.map(Box::new),
    ..ChatAction::new(name, None, outcome)
  };
  keep::keep(scope, (None, None), action).await;
  runners.chat_changed(&scope.root);
  result
}

/// What a call did: a card for the person, or a narrowing made at once with what the tool answered.
enum Acted {
  Proposed(ProposedChange),
  Made(Result<Value, ToolError>, ProposedChange),
}

async fn act(
  (services, translate): (&Services, &Translate),
  scope: &ChatScope,
  name: &str,
  arguments: JsonObject,
) -> Result<Acted, ToolError> {
  let operation = operation(name, &arguments)?;
  let read = services.clone();
  let mcp = blocking(move || preview_mcp(&read, operation, &arguments))
    .await?
    .change;
  // Nobody asked for anything in a turn Orchester started: even a narrowing waits for the person there.
  let at_once = !mcp.widens && !scope.on_its_own && changes_permissions(services).await?;
  let made = if at_once {
    Some(app_tool((services, translate), setup_tool(operation), mcp.arguments.clone()).await)
  } else {
    None
  };
  let change = ProposedChange {
    subject: mcp.server.name.clone(),
    change: Change::Mcp(Box::new(mcp)),
  };
  Ok(match made {
    Some(made) => Acted::Made(made, change),
    None => Acted::Proposed(change),
  })
}

/// Applies `card`, an MCP server's change the person applied: refused unless it is still what the card shows and
/// stands on, read again just now; then carried out by the app's own tool as the person's decision, what it widens
/// theirs as they were shown it.
pub(super) async fn apply(
  (services, translate): (&Services, &Translate),
  card: &McpChange,
) -> Result<Value, ToolError> {
  let (read, operation, arguments) = (services.clone(), card.operation, card.arguments.clone());
  let now = blocking(move || preview_mcp(&read, operation, &arguments)).await?;
  if now.change != *card {
    return Err(ToolError::refused(
      "changed_since_proposed",
      vec![Message::new("orchestrator.mcp.changed").with("server", card.server.name.clone())],
    ));
  }
  let person = services.clone().decided_by_person(PersonDecision {
    draft: None,
    widens: now.widens,
  });
  app_tool((&person, translate), setup_tool(operation), card.arguments.clone()).await
}

/// What the agent is told of the proposed `change`, and what it proposes.
pub(super) fn told(change: &McpChange) -> (Message, Value) {
  let message = Message::new("orchestrator.tools.mcpWaits").with("server", change.server.name.clone());
  let proposed = json!({
    "mcp_server": { "operation": change.operation, "server": &change.server, "widens": change.widens },
  });
  (message, proposed)
}

/// What the chat's tool `name` does with `arguments`.
fn operation(name: &str, arguments: &JsonObject) -> Result<McpOperation, ToolError> {
  match name {
    "mcp_add_server" => Ok(McpOperation::Add),
    "mcp_update_server" => Ok(McpOperation::Update),
    "mcp_remove_server" => Ok(McpOperation::Remove),
    "mcp_set_server_enabled" => match arguments.get("enabled").and_then(Value::as_bool) {
      Some(true) => Ok(McpOperation::Enable),
      Some(false) => Ok(McpOperation::Disable),
      None => Err(ToolError::InvalidArguments(
        "enabled: true or false is required".to_owned(),
      )),
    },
    other => Err(ToolError::InvalidArguments(format!(
      "no MCP server tool is named {other}"
    ))),
  }
}

/// The app's own tool that carries out `operation`.
const fn setup_tool(operation: McpOperation) -> &'static str {
  match operation {
    McpOperation::Add => "add_mcp_server",
    McpOperation::Update => "update_mcp_server",
    McpOperation::Enable | McpOperation::Disable => "set_mcp_server_enabled",
    McpOperation::Remove => "remove_mcp_server",
  }
}
