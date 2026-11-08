//! What the Agents hub of a local project lists (`Agents · v2` `acFkz`, L1–L14; `docs/design/team-configuration.md`
//! rules 11 and 12): the readiness of its profiles and tools ([`crate::readiness`]), its local MCP servers each on
//! their own, its instructions and skills, which need nothing of a computer, and what was archived. Nothing here is the
//! team's: no split, no versions of an MCP server (a local one is a connection, edited in place).

use std::path::Path;

use orchester_core::{
  connection::{AgentProvider, ConnectionKind, mcp::McpTransport},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, Instruction, Pipeline, ResourceKind, Skill, Tool, ToolKind, VersionNumber},
  secret::SecretVault,
  store::{ConnectionStore, McpServerStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::team_hub::Form;
use crate::{
  local_archive::Archivable,
  readiness::{LocalMcpReadiness, ProjectReadiness, ReadinessError},
  workspace::Workspace,
};

/// An instruction or a skill of the project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalPlain {
  pub id: ResourceId,
  pub kind: ResourceKind,
  pub name: String,
  pub number: VersionNumber,
}

/// What a row and a panel of a local profile say beyond how it stands: the CLI it runs with, the connection it runs on,
/// the MCP servers it chooses and the pipelines whose steps run it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalProfile {
  /// The CLI it runs with; `None` when the connection it names is gone or is not an agent connection.
  pub cli: Option<AgentProvider>,
  /// The connection it runs on, by its name; `None` when the project no longer has it.
  pub connection: Option<String>,
  pub mcp_servers: Vec<(ConnectionId, String)>,
  /// The pipelines, by name, whose latest version has a step that runs it.
  pub pipelines: Vec<String>,
}

/// A local resource or MCP server that was archived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalArchived {
  pub what: Archivable,
  /// What it is: a resource's kind, or [`ResourceKind::McpServer`].
  pub kind: ResourceKind,
  pub name: String,
  pub at: Timestamp,
}

/// The resources the hub of a local project lists, and how they stand on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalHub {
  readiness: ProjectReadiness,
  mcp_servers: Vec<LocalMcpReadiness>,
  plain: Vec<LocalPlain>,
  profiles: Vec<(ResourceId, LocalProfile)>,
  tool_forms: Vec<(ResourceId, Form)>,
  archived: Vec<LocalArchived>,
}

impl LocalHub {
  /// The profiles and tools with how they stand here, and the pipelines.
  pub const fn readiness(&self) -> &ProjectReadiness {
    &self.readiness
  }

  /// The local MCP servers that are not archived, by name.
  pub fn mcp_servers(&self) -> &[LocalMcpReadiness] {
    &self.mcp_servers
  }

  pub fn mcp_server(&self, id: ConnectionId) -> Option<&LocalMcpReadiness> {
    self.mcp_servers.iter().find(|found| found.id() == id)
  }

  /// The instructions, then the skills, each by name.
  pub fn plain(&self) -> &[LocalPlain] {
    &self.plain
  }

  pub fn profile(&self, id: ResourceId) -> Option<&LocalProfile> {
    self
      .profiles
      .iter()
      .find_map(|(found, profile)| (*found == id).then_some(profile))
  }

  /// What a tool is; `None` for another resource.
  pub fn tool_form(&self, id: ResourceId) -> Option<Form> {
    self
      .tool_forms
      .iter()
      .find_map(|(found, form)| (*found == id).then_some(*form))
  }

  /// What was archived, newest first.
  pub fn archived(&self) -> &[LocalArchived] {
    &self.archived
  }

  /// The names of the profiles that choose the MCP server `id`, by name.
  pub fn profiles_choosing(&self, id: ConnectionId) -> Vec<&str> {
    let mut names: Vec<&str> = self
      .readiness
      .resources()
      .iter()
      .filter(|found| found.kind() == ResourceKind::AgentProfile)
      .filter(|found| {
        self
          .profile(found.resource())
          .is_some_and(|profile| profile.mcp_servers.iter().any(|(server, _)| *server == id))
      })
      .map(|found| found.name())
      .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names
  }
}

impl Workspace {
  /// The hub of the local project at `repo_root`. Reads the database and the keychain's list of what it has, so it
  /// blocks: run it on the background pool, on an explicit change and never on a timer
  /// ([`Workspace::project_readiness`]).
  pub fn local_hub(&self, repo_root: &Path, vault: &dyn SecretVault) -> Result<LocalHub, ReadinessError> {
    let db = ProjectDb::open(repo_root)?;
    let readiness = self.readiness_in(&db, repo_root, vault)?;
    Ok(LocalHub {
      readiness,
      mcp_servers: crate::readiness::local_mcp_servers(&db, repo_root, vault)?,
      plain: plain(&db)?,
      profiles: profiles(&db)?,
      tool_forms: tool_forms(&db)?,
      archived: archived(&db)?,
    })
  }
}

fn plain(db: &ProjectDb) -> Result<Vec<LocalPlain>, StoreError> {
  let mut plain = Vec::new();
  for kind in [ResourceKind::Instruction, ResourceKind::Skill] {
    let mut of_kind = Vec::new();
    for resource in db.resources(kind)? {
      let number = match kind {
        ResourceKind::Instruction => db.latest_version::<Instruction>(resource.id())?.map(|v| v.number()),
        _ => db.latest_version::<Skill>(resource.id())?.map(|v| v.number()),
      };
      if let Some(number) = number {
        of_kind.push(LocalPlain {
          id: resource.id(),
          kind,
          name: resource.name().to_string(),
          number,
        });
      }
    }
    of_kind.sort_by_key(|found| found.name.to_lowercase());
    plain.extend(of_kind);
  }
  Ok(plain)
}

fn profiles(db: &ProjectDb) -> Result<Vec<(ResourceId, LocalProfile)>, StoreError> {
  let servers = db.mcp_servers()?;
  let pipelines = latest_pipelines(db)?;
  let mut profiles = Vec::new();
  for resource in db.resources(ResourceKind::AgentProfile)? {
    let Some(version) = db.latest_version::<AgentProfile>(resource.id())? else {
      continue;
    };
    let connection = db.connection(version.body().connection())?;
    let cli = match connection.as_ref().map(orchester_core::connection::Connection::kind) {
      Some(ConnectionKind::Agent(cli)) => Some(*cli),
      _ => None,
    };
    let mut running: Vec<String> = pipelines
      .iter()
      .filter(|(_, pipeline)| pipeline.steps_run_by(resource.id()).next().is_some())
      .map(|(name, _)| name.clone())
      .collect();
    running.sort_by_key(|name| name.to_lowercase());
    profiles.push((
      resource.id(),
      LocalProfile {
        cli,
        connection: connection.map(|connection| connection.name().to_string()),
        mcp_servers: version
          .body()
          .mcp_servers()
          .ids()
          .iter()
          .filter_map(|id| servers.iter().find(|server| server.id() == *id))
          .map(|server| (server.id(), server.name().to_string()))
          .collect(),
        pipelines: running,
      },
    ));
  }
  Ok(profiles)
}

fn latest_pipelines(db: &ProjectDb) -> Result<Vec<(String, Pipeline)>, StoreError> {
  let mut found = Vec::new();
  for resource in db.resources(ResourceKind::Pipeline)? {
    if let Some(version) = db.latest_version::<Pipeline>(resource.id())? {
      found.push((resource.name().to_string(), version.body().clone()));
    }
  }
  Ok(found)
}

fn tool_forms(db: &ProjectDb) -> Result<Vec<(ResourceId, Form)>, StoreError> {
  let mut forms = Vec::new();
  for resource in db.resources(ResourceKind::Tool)? {
    if let Some(version) = db.latest_version::<Tool>(resource.id())? {
      forms.push((
        resource.id(),
        match version.body().kind() {
          ToolKind::Command => Form::ToolCommand,
          ToolKind::Http => Form::ToolHttp,
          ToolKind::Database => Form::ToolDatabase,
        },
      ));
    }
  }
  Ok(forms)
}

fn archived(db: &ProjectDb) -> Result<Vec<LocalArchived>, StoreError> {
  let mut archived = Vec::new();
  for kind in [
    ResourceKind::AgentProfile,
    ResourceKind::Instruction,
    ResourceKind::Skill,
    ResourceKind::Tool,
  ] {
    for (resource, at) in db.archived_local_resources(kind)? {
      archived.push(LocalArchived {
        what: Archivable::Resource(resource.id()),
        kind,
        name: resource.name().to_string(),
        at,
      });
    }
  }
  let servers = db.mcp_servers()?;
  for (id, at) in db.archived_mcp_servers()? {
    if let Some(server) = servers.iter().find(|server| server.id() == id) {
      archived.push(LocalArchived {
        what: Archivable::McpServer(id),
        kind: ResourceKind::McpServer,
        name: server.name().to_string(),
        at,
      });
    }
  }
  archived.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.name.cmp(&b.name)));
  Ok(archived)
}

/// Whether a server is reached at an address, from its transport.
pub fn mcp_form(transport: &McpTransport) -> Form {
  match transport {
    McpTransport::Stdio(_) => Form::McpProgram,
    McpTransport::Http(_) => Form::McpRemote,
  }
}
