//! The Agents hub of a local project (ORC-842, `Agents · v2` L1–L14): Kontur with its connections, its MCP servers (one
//! with no program path, one off, one remote) and the catalogue's profiles, which choose kontur-docs.
//!
//! - `local-hub`: the hub on All; *MCP servers* lists the servers (L2), a server is edited in place (L8, L12).
//! - `local-hub-archive`: the same with two runs working on Feature delivery, so *Archive* on Builder or Reviewer lists
//!   what names it and the runs that will stop (L11).
//! - `local-hub-claude`: the same with two Claude Code connections, Writer on Claude 2 and Builder whose connection is
//!   gone, so it asks which one runs it (L1).
//! - `local-hub-archived`: the same after archiving Reviewer, a skill and postgres-dev, each with the runs it stopped
//!   (L13, L14).

use std::collections::BTreeMap;

use orchester_core::{
  connection::{
    AgentProvider, SignIn,
    mcp::{McpEnvValue, McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer},
  },
  id::{ConnectionId, ResourceId, RunId, SecretId, TaskId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{
    AgentProfile, CommandArg, CommandProgram, Draft, InstructionScope, ModelId, PromptText, ProviderSettings,
    RepositoryAccess, ResourceKind, VariableName,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{McpServerStore, ResourceStore, SecretStore},
};
use orchester_store::{ArchiveStop, Archived};
use rusqlite::Connection;

use super::{
  agents_library::{instruction, publish, skill},
  connections, qa_app_asks,
};
use crate::kontur::{
  Kontur, SeedError,
  catalogue::Catalogue,
  name,
  shapes::{self, Shape},
};

fn invalid(error: &dyn std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  base(kontur).map(drop)
}

/// The connections, the servers and the catalogue, whose Feature delivery the runs of the other scenarios work on.
fn base(kontur: &mut Kontur) -> Result<Catalogue, SeedError> {
  let catalogue = super::connections::seeded(kontur)?.catalogue;
  let at = kontur.ago(60);
  let transport = McpTransport::Stdio(StdioServer::new(
    CommandProgram::new("pencil-mcp-not-installed").map_err(|error| invalid(&error))?,
    Vec::new(),
    None,
  ));
  let pencil = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("pencil-design").map_err(|error| invalid(&error))?,
    transport,
    McpEnvironment::default(),
    at,
  )
  .map_err(|error| invalid(&error))?;
  kontur.db.create_mcp_server(&pencil)?;
  choose_docs(kontur)?;
  publish(
    kontur,
    "Project rules",
    vec![instruction("Keep each change small.", InstructionScope::Project)?],
  )?;
  publish(
    kontur,
    "run-checks",
    vec![skill(
      "Runs the project's checks before a commit.",
      "Run `cargo test` and fix what fails.",
      InstructionScope::Project,
    )?],
  )?;
  Ok(catalogue)
}

/// `local-hub-secret`: the hub with figma, a program that runs when the name `npx` is found on this computer and whose
/// variable `FIGMA_API_KEY` takes the secret `FIGMA_TOKEN`, which has no value yet: in its panel *Replace* (*Set
/// value*) types one and *Save* keeps it in the QA app's keychain in memory, which accepts (L8).
pub fn secret(kontur: &mut Kontur) -> Result<(), SeedError> {
  base(kontur)?;
  figma(kontur).map(drop)
}

/// `local-hub-secret-refused`: [`secret`] with a QA app whose secret store refuses every value, so *Save* with a typed
/// value is refused as L12 draws it and nothing is saved, not even the other fields.
pub fn secret_refused(kontur: &mut Kontur) -> Result<(), SeedError> {
  secret(kontur)?;
  kontur.ask_qa_app(qa_app_asks::SECRET_STORE_REFUSES)
}

/// figma, with `FIGMA_API_KEY` taking the secret `FIGMA_TOKEN`.
fn figma(kontur: &mut Kontur) -> Result<ConnectionId, SeedError> {
  let at = kontur.ago(50);
  let token = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("FIGMA_TOKEN").map_err(|error| invalid(&error))?,
      kind: SecretKind::AccessToken,
      environment: Environment::Development,
    },
    at,
  );
  kontur.db.create_secret(&token)?;
  let transport = McpTransport::Stdio(StdioServer::new(
    CommandProgram::new("npx").map_err(|error| invalid(&error))?,
    ["figma-developer-mcp", "--stdio"]
      .iter()
      .map(|arg| CommandArg::new(arg).map_err(|error| invalid(&error)))
      .collect::<Result<Vec<_>, _>>()?,
    None,
  ));
  let variables = McpEnvironment::new(BTreeMap::from([(
    VariableName::new("FIGMA_API_KEY").map_err(|error| invalid(&error))?,
    McpEnvValue::Secret(token.id()),
  )]))
  .map_err(|error| invalid(&error))?;
  let figma = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("figma").map_err(|error| invalid(&error))?,
    transport,
    variables,
    at,
  )
  .map_err(|error| invalid(&error))?;
  kontur.db.create_mcp_server(&figma)?;
  Ok(figma.id())
}

/// `local-hub-claude`: Writer runs on the Claude Code connection Claude 2, and Home is another one to choose; each
/// connection was checked at a version of its own, so the pick lists them apart (L1).
pub fn claude(kontur: &mut Kontur) -> Result<(), SeedError> {
  base(kontur)?;
  let bin = kontur.repo.join(".orchester").join("qa-bin");
  let program = |file: &str| -> Result<std::path::PathBuf, SeedError> {
    let path = bin.join(file);
    std::fs::write(&path, "").map_err(|error| invalid(&error))?;
    Ok(path)
  };
  let at = kontur.ago(4);
  let models = ["claude-opus-5-5", "claude-sonnet-5-5"];
  let (work, programs) = (program("claude-2")?, program("claude-home")?);
  let claude_2 = connections::cli(
    kontur,
    ("Claude 2", AgentProvider::ClaudeCodeCli, work),
    ("2.1.173", SignIn::SignedIn, &models),
    at,
  )?;
  connections::cli(
    kontur,
    ("Home", AgentProvider::ClaudeCodeCli, programs),
    ("2.1.170", SignIn::SignedIn, &models),
    at,
  )?;
  let writer = AgentProfile::new(
    name("Writer"),
    "Writes the release notes.".to_owned(),
    PromptText::new("Writes the release notes from the merged tasks.").map_err(|error| invalid(&error))?,
    claude_2,
    ModelId::new("claude-sonnet-5-5").map_err(|error| invalid(&error))?,
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  ));
  publish(kontur, "Writer", vec![writer])?;
  builder_without_connection(kontur)
}

/// Builder's next version names a Claude model on a connection this project no longer has, and chooses pencil-design
/// (no program path) and sentry (a remote server whose secret has no value): the profile asks which of the two Claude
/// Code connections runs it (L1). Its permissions are what Claude Code enforces on every platform, so choosing a
/// connection can publish it.
fn builder_without_connection(kontur: &mut Kontur) -> Result<(), SeedError> {
  let servers = kontur.db.mcp_servers()?;
  let chosen = ["pencil-design", "sentry"]
    .iter()
    .map(|called| {
      servers
        .iter()
        .find(|server| server.name().as_str() == *called)
        .map(McpServer::id)
        .ok_or_else(|| SeedError::Invalid(format!("the seed has the MCP server {called}")))
    })
    .collect::<Result<Vec<_>, _>>()?;
  let builder = kontur
    .db
    .resources(ResourceKind::AgentProfile)?
    .into_iter()
    .find(|resource| resource.name().as_str() == "Builder")
    .ok_or_else(|| SeedError::Invalid("the seed has the profile Builder".to_owned()))?;
  let body = AgentProfile::new(
    name("Builder"),
    "Builds what a task asks for.".to_owned(),
    PromptText::new("Builds the change a task asks for, in small steps.").map_err(|error| invalid(&error))?,
    ConnectionId::generate(),
    ModelId::new("claude-opus-5-5").map_err(|error| invalid(&error))?,
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  ))
  .with_mcp_servers(McpServerSelection::new(chosen).map_err(|error| invalid(&error))?);
  let latest = kontur
    .db
    .latest_version::<AgentProfile>(builder.id())?
    .map(|version| version.number());
  kontur
    .db
    .save_draft(builder.id(), &Draft::new(body, latest, kontur.ago(3)))?;
  kontur.db.publish_draft::<AgentProfile>(builder.id(), kontur.ago(3))?;
  Ok(())
}

/// Builder and Reviewer choose kontur-docs from their next version on, so the server says who uses it.
fn choose_docs(kontur: &mut Kontur) -> Result<(), SeedError> {
  let docs = kontur
    .db
    .mcp_servers()?
    .into_iter()
    .find(|server| server.name().as_str() == "kontur-docs")
    .ok_or_else(|| SeedError::Invalid("the seed has the MCP server kontur-docs".to_owned()))?;
  let servers = McpServerSelection::new(vec![docs.id()]).map_err(|error| invalid(&error))?;
  for resource in kontur.db.resources(ResourceKind::AgentProfile)? {
    let Some(version) = kontur.db.latest_version::<AgentProfile>(resource.id())? else {
      continue;
    };
    if !matches!(version.body().role().as_str(), "Builder" | "Reviewer") {
      continue;
    }
    let body = version.body().clone().with_mcp_servers(servers.clone());
    kontur
      .db
      .save_draft(resource.id(), &Draft::new(body, Some(version.number()), kontur.ago(30)))?;
    kontur.db.publish_draft::<AgentProfile>(resource.id(), kontur.ago(30))?;
  }
  Ok(())
}

pub fn archive(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = base(kontur)?;
  with_runs(
    kontur,
    &catalogue,
    &[(42, "Add retry to webhook sender", Shape::Running("implement"), 20)],
  )?;
  Ok(())
}

pub fn archived(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = base(kontur)?;
  let runs = with_runs(
    kontur,
    &catalogue,
    &[
      (9, "Design the settings page", Shape::Cancelled("implement"), 90),
      (11, "Review the webhook change", Shape::Cancelled("review"), 80),
    ],
  )?;
  let archived_at = kontur.ago(70);
  let find = |kontur: &Kontur, kind, called: &str| -> Result<ResourceId, SeedError> {
    kontur
      .db
      .resources(kind)?
      .into_iter()
      .find(|resource| resource.name().as_str() == called)
      .map(|resource| resource.id())
      .ok_or_else(|| SeedError::Invalid(format!("the seed has {called}")))
  };
  let reviewer = find(kontur, ResourceKind::AgentProfile, "Reviewer")?;
  stop(
    kontur,
    (runs[1].0, runs[1].1),
    "Reviewer",
    Some((reviewer, ResourceKind::AgentProfile)),
  )?;
  kontur.db.archive_local_resource(reviewer, archived_at)?;
  let docs = kontur
    .db
    .mcp_servers()?
    .into_iter()
    .find(|server| server.name().as_str() == "postgres-dev")
    .ok_or_else(|| SeedError::Invalid("the seed has the MCP server postgres-dev".to_owned()))?;
  stop(kontur, (runs[0].0, runs[0].1), "postgres-dev", None)?;
  kontur.db.record_mcp_server_stop(runs[0].0, docs.id())?;
  kontur.db.archive_mcp_server(docs.id(), kontur.ago(75))?;
  if let Some(skill) = kontur.db.resources(ResourceKind::Skill)?.into_iter().next() {
    kontur.db.archive_local_resource(skill.id(), kontur.ago(80))?;
  }
  Ok(())
}

/// Records that `run` of `task` was stopped when `called` was archived.
fn stop(
  kontur: &mut Kontur,
  (run, task): (RunId, TaskId),
  called: &str,
  resource: Option<(ResourceId, ResourceKind)>,
) -> Result<(), SeedError> {
  kontur.db.record_archive_stop(&ArchiveStop {
    run,
    task,
    resource: name(called),
    stopped_at: kontur.ago(70),
    archived: resource.map(|(id, kind)| Archived { id, kind, by: None }),
  })?;
  Ok(())
}

/// Kontur's tasks numbered as `runs` say, each with its title and its run in its shape, started minutes ago; each run
/// with its task.
fn with_runs(
  kontur: &mut Kontur,
  catalogue: &Catalogue,
  runs: &[(u32, &str, Shape, i64)],
) -> Result<Vec<(RunId, TaskId)>, SeedError> {
  let pipeline = &catalogue.feature_delivery;
  let mut made = Vec::new();
  for &(number, title, shape, minutes) in runs {
    Connection::open(kontur.database())?.execute("UPDATE counters SET next = ?1 WHERE name = 'task'", [number])?;
    let (created, started) = (kontur.ago(minutes + 30), kontur.ago(minutes));
    let mut task = kontur.task((title, title, &[title]), Some(pipeline), created)?;
    shapes::apply(kontur, &mut task, Some(pipeline), shape, started)?;
    let run: String = Connection::open(kontur.database())?.query_row(
      "SELECT id FROM runs WHERE task_id = ?1 ORDER BY number DESC LIMIT 1",
      [task.id().to_string()],
      |row| row.get(0),
    )?;
    made.push((run.parse().map_err(|error| invalid(&error))?, task.id()));
  }
  Ok(made)
}
