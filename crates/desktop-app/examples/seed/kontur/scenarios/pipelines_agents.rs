//! The step's Agent field and Agent picker with every kind of agent (`Pipelines · v2` P2 `u2rYEH`, P10 `HjgZ8`, K4
//! `CwLfL`, PV2-08): one published pipeline, *Agents tour*, whose eight steps each run one of them. No agent or command
//! is started.
//!
//! - *Plan*: Builder on the Claude Code connection Work (`claude-opus-5-5`), which is ready: the Claude mark and
//!   *Claude Code · Work · claude-opus-5-5* with a clean field.
//! - *Notes*: Release notes on the Codex connection Codex main (`gpt-6.1-sol`): the OpenAI mark.
//! - *Write*: Writer chooses an MCP server whose program is not on this computer, so it is not ready for a reason that
//!   is not a connection (*Set the program path of figma*).
//! - *Explore*: Explorer runs on a connection the project no longer has (*Choose a connection*).
//! - *Checks*: the command tool Checks (`npm test`), and *Lint*: a tool with a command too long for its row.
//! - *Override*: Planner with the step's own model, `claude-sonnet-5-5`, which the field and the picker's chosen row
//!   say after the profile's line (*· model: claude-sonnet-5-5*).
//! - *Retired*: a profile that was archived after the step named it, which the project no longer lists.

use std::collections::BTreeMap;

use orchester_core::{
  connection::{
    AgentEndpoint, AgentProvider, Connection, ConnectionKind,
    mcp::{McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer},
  },
  id::{ConnectionId, ResourceId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, StepPermissions},
  resource::{
    AgentProfile, CommandAction, CommandArg, CommandProgram, ModelId, Pipeline, PipelineEdit, PromptText,
    ProviderSettings, RepositoryAccess, Slug, StepKind, Tool, ToolAction,
  },
  store::{ConnectionStore, McpServerStore, ResourceStore},
};

use super::pipelines::create;
use crate::kontur::{Kontur, SeedError, name};

fn invalid(error: &dyn std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let work = connection(kontur, "Work", AgentProvider::ClaudeCodeCli)?;
  let codex = connection(kontur, "Codex main", AgentProvider::CodexCli)?;
  let figma = figma(kontur)?;
  let builder = profile(kontur, "Builder", (work, "claude-opus-5-5"), None)?;
  let notes = profile(kontur, "Release notes", (codex, "gpt-6.1-sol"), None)?;
  let writer = profile(kontur, "Writer", (work, "claude-opus-5-5"), Some(figma))?;
  let explorer = profile(kontur, "Explorer", (ConnectionId::generate(), "claude-opus-5-5"), None)?;
  let planner = profile(kontur, "Planner", (work, "claude-opus-5-5"), None)?;
  let retired = profile(kontur, "Retired reviewer", (work, "claude-opus-5-5"), None)?;
  let checks = tool(kontur, "Checks", "npm", &["test"])?;
  let lint = tool(
    kontur,
    "Lint",
    "cargo",
    &[
      "clippy",
      "--workspace",
      "--all-targets",
      "--all-features",
      "--",
      "-D",
      "warnings",
    ],
  )?;

  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty draft"));
  let mut before = None;
  for (step, kind) in [
    ("Plan", agent(builder, None)),
    ("Notes", agent(notes, None)),
    ("Write", agent(writer, None)),
    ("Explore", agent(explorer, None)),
    ("Checks", StepKind::Tool { tool: Some(checks) }),
    ("Lint", StepKind::Tool { tool: Some(lint) }),
    (
      "Override",
      agent(
        planner,
        Some(ModelId::new("claude-sonnet-5-5").map_err(|error| invalid(&error))?),
      ),
    ),
    ("Retired", agent(retired, None)),
  ] {
    let key = edit.add_step(name(step), before.as_ref());
    edit.update(&key, |step| step.kind = kind);
    before = Some(key);
  }
  let tour = create(
    kontur,
    "Agents tour",
    &edit.validate().expect("steps with executors are valid"),
  )?;
  kontur.db.publish_draft::<Pipeline>(tour, kontur.now)?;
  // After the step names it: the project no longer lists the profile, and the step keeps naming it.
  kontur.db.archive_local_resource(retired, kontur.now)?;
  Ok(())
}

fn agent(profile: ResourceId, model: Option<ModelId>) -> StepKind {
  StepKind::Agent {
    profile: Some(profile),
    repository: RepositoryAccess::ReadOnly,
    model,
    permissions: StepPermissions::default(),
  }
}

/// An agent connection called `title` to `provider`, whose program is never started.
fn connection(kontur: &mut Kontur, title: &str, provider: AgentProvider) -> Result<ConnectionId, SeedError> {
  let id = ConnectionId::generate();
  let program = match provider {
    AgentProvider::CodexCli => ".orchester/unused-qa-codex",
    _ => ".orchester/unused-qa-claude",
  };
  kontur.db.create_connection(&Connection::new(
    id,
    name(title),
    ConnectionKind::Agent(provider),
    AgentEndpoint::Program(kontur.repo.join(program)),
    kontur.now,
  )?)?;
  Ok(id)
}

/// An MCP server `figma` that starts a program this computer does not have.
fn figma(kontur: &mut Kontur) -> Result<ConnectionId, SeedError> {
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("figma").map_err(|error| invalid(&error))?,
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("figma-mcp-not-installed").map_err(|error| invalid(&error))?,
      Vec::new(),
      None,
    )),
    McpEnvironment::new(BTreeMap::new()).map_err(|error| invalid(&error))?,
    kontur.now,
  )
  .map_err(|error| invalid(&error))?;
  kontur.db.create_mcp_server(&server)?;
  Ok(server.id())
}

/// A published profile called `title` that runs `model` on `connection`, choosing the MCP server `server` if any.
fn profile(
  kontur: &mut Kontur,
  title: &str,
  (connection, model): (ConnectionId, &str),
  server: Option<ConnectionId>,
) -> Result<ResourceId, SeedError> {
  let chosen = McpServerSelection::new(server.into_iter().collect()).map_err(|error| invalid(&error))?;
  let body = AgentProfile::new(
    name(title),
    String::new(),
    PromptText::new("").map_err(|error| invalid(&error))?,
    connection,
    ModelId::new(model).map_err(|error| invalid(&error))?,
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  ))
  .with_mcp_servers(chosen);
  let id = kontur
    .db
    .create_resource(
      Slug::from_name(title).ok_or_else(|| invalid(&"the title makes no slug"))?,
      name(title),
      &body,
      kontur.now,
    )?
    .id();
  kontur.db.publish_draft::<AgentProfile>(id, kontur.now)?;
  Ok(id)
}

/// A published command tool called `title` that runs `program` with `arguments`.
fn tool(kontur: &mut Kontur, title: &str, program: &str, arguments: &[&str]) -> Result<ResourceId, SeedError> {
  let arguments = arguments
    .iter()
    .map(|argument| CommandArg::new(argument).map_err(|error| invalid(&error)))
    .collect::<Result<Vec<_>, _>>()?;
  let body = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(
      CommandProgram::new(program).map_err(|error| invalid(&error))?,
      arguments,
    )),
    None,
  )
  .map_err(|error| invalid(&error))?;
  let id = kontur
    .db
    .create_resource(
      Slug::from_name(title).ok_or_else(|| invalid(&"the title makes no slug"))?,
      name(title),
      &body,
      kontur.now,
    )?
    .id();
  kontur.db.publish_draft::<Tool>(id, kontur.now)?;
  Ok(id)
}
