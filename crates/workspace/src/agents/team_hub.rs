//! What the Agents hub of a team project lists (`Agents · v2` `acFkz`, ORC-831): the readiness of its profiles, MCP
//! servers and tools ([`crate::readiness`]), and its instructions and skills, which need nothing of a computer and so
//! are always ready here (`docs/design/team-configuration.md`). All of it is read from the team's configuration as the
//! team server was read this session ([`crate::team_config`]), with what this computer filled in.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, TeamMcpLaunch, TeamMcpServer, ToolKind, VersionNumber},
  secret::SecretVault,
};
use orchester_store::ProjectDb;

use super::{
  team_archived::{self, TeamArchived},
  team_mcp::{self, TeamMcpSheet},
  team_profile::{self, TeamProfileSheet},
  team_text::{self, TeamInstructionSheet, TeamSkillSheet},
  team_tool::{self, TeamToolSheet},
};
use crate::{
  readiness::{ProjectReadiness, ReadinessError},
  team_config::{TeamConfig, TeamRead, Unreached},
  workspace::Workspace,
};

/// How the hub of a team project stands this session (`Agents · v2` `acFkz`): its team server is being read, could not
/// be reached while nothing was read (A25 `Zhcif`), or was read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamHubRead {
  Reading,
  Unreachable(Unreached),
  Read(Box<TeamHub>),
}

/// An instruction or a skill of the team.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plain {
  pub id: ResourceId,
  pub kind: ResourceKind,
  pub name: String,
  /// Its latest version; `None` while the server has numbered none.
  pub number: Option<VersionNumber>,
}

/// What an MCP server or a tool is, which a row of the hub names after its name (`Tag` in `Item` of `Agents · MCP
/// servers` `yIsDi`, `Agents · Tools` `u9Hcc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
  /// An MCP server that runs a program on each computer.
  McpProgram,
  /// An MCP server that is reached at an address.
  McpRemote,
  ToolCommand,
  ToolHttp,
  ToolDatabase,
}

/// The resources the hub lists, and how the profiles, MCP servers and tools stand on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamHub {
  /// Why the team server cannot be reached now, while it cannot: what was read this session is shown, and nothing is
  /// saved to it.
  unreached: Option<Unreached>,
  readiness: ProjectReadiness,
  plain: Vec<Plain>,
  forms: Vec<(ResourceId, Form)>,
  profiles: Vec<(ResourceId, TeamProfileSheet)>,
  mcp_servers: Vec<(ResourceId, TeamMcpSheet)>,
  tools: Vec<(ResourceId, TeamToolSheet)>,
  instructions: Vec<(ResourceId, TeamInstructionSheet)>,
  skills: Vec<(ResourceId, TeamSkillSheet)>,
  archived: Vec<TeamArchived>,
}

impl TeamHub {
  /// Why the team server cannot be reached now; `None` while it can.
  pub const fn unreached(&self) -> Option<&Unreached> {
    self.unreached.as_ref()
  }

  pub const fn readiness(&self) -> &ProjectReadiness {
    &self.readiness
  }

  /// The team's instructions, then its skills, each by name.
  pub fn plain(&self) -> &[Plain] {
    &self.plain
  }

  /// What the team holds of the profile `resource` and what this computer fills in for it; `None` for any other
  /// resource, and for a profile whose latest version is not in the team's form.
  pub fn profile(&self, resource: ResourceId) -> Option<&TeamProfileSheet> {
    found(&self.profiles, resource)
  }

  /// What the team holds of the MCP server `resource` and what this computer fills in for it; `None` for any other
  /// resource, and for a server whose latest version this computer cannot read.
  pub fn mcp_server(&self, resource: ResourceId) -> Option<&TeamMcpSheet> {
    found(&self.mcp_servers, resource)
  }

  /// What the team holds of the tool `resource`; `None` for any other resource, and for a tool whose latest version
  /// this computer cannot read.
  pub fn tool(&self, resource: ResourceId) -> Option<&TeamToolSheet> {
    found(&self.tools, resource)
  }

  /// What the team holds of the instruction `resource`; `None` for any other resource, and for one whose latest
  /// version this computer cannot read.
  pub fn instruction(&self, resource: ResourceId) -> Option<&TeamInstructionSheet> {
    found(&self.instructions, resource)
  }

  /// What the team holds of the skill `resource`; `None` for any other resource, and for one whose latest version this
  /// computer cannot read.
  pub fn skill(&self, resource: ResourceId) -> Option<&TeamSkillSheet> {
    found(&self.skills, resource)
  }

  /// What the team archived, the latest archived first (`Agents · Archived · admin · Restore` A36 `qvMZD`).
  pub fn archived(&self) -> &[TeamArchived] {
    &self.archived
  }

  /// What the MCP server or tool `resource` is; `None` for any other resource.
  pub fn form(&self, resource: ResourceId) -> Option<Form> {
    found(&self.forms, resource).copied()
  }
}

fn found<T>(sheets: &[(ResourceId, T)], resource: ResourceId) -> Option<&T> {
  sheets.iter().find_map(|(id, sheet)| (*id == resource).then_some(sheet))
}

/// What each MCP server and tool of the project is, from its latest version.
fn forms(config: &TeamConfig) -> Vec<(ResourceId, Form)> {
  let mut forms = Vec::new();
  for resource in config.resources(ResourceKind::McpServer) {
    if let Some(Ok(body)) = resource.body::<TeamMcpServer>() {
      let form = match body.launch() {
        TeamMcpLaunch::Program(_) => Form::McpProgram,
        TeamMcpLaunch::Remote(_) => Form::McpRemote,
      };
      forms.push((resource.id, form));
    }
  }
  for resource in config.resources(ResourceKind::Tool) {
    if let Some(kind) = team_tool::kind(resource) {
      let form = match kind {
        ToolKind::Command => Form::ToolCommand,
        ToolKind::Http => Form::ToolHttp,
        ToolKind::Database => Form::ToolDatabase,
      };
      forms.push((resource.id, form));
    }
  }
  forms
}

/// The team's instructions, then its skills, each by name.
fn plain(config: &TeamConfig) -> Vec<Plain> {
  [ResourceKind::Instruction, ResourceKind::Skill]
    .into_iter()
    .flat_map(|kind| {
      config.resources(kind).into_iter().map(move |resource| Plain {
        id: resource.id,
        kind,
        name: resource.name.as_str().to_owned(),
        number: resource.number(),
      })
    })
    .collect()
}

impl Workspace {
  /// The hub of the team project at `repo_root`, from its configuration as the team server was read this session and
  /// what this computer filled in. Reads the databases and the keychain's list of what it has, so it blocks: run it on
  /// the background pool, on an explicit change and never on a timer ([`Workspace::project_readiness`]).
  pub fn team_hub(&self, repo_root: &Path, vault: &dyn SecretVault) -> Result<TeamHubRead, ReadinessError> {
    let (config, unreached) = match self.team.read(repo_root) {
      TeamRead::Reading => return Ok(TeamHubRead::Reading),
      TeamRead::Unreachable(unreached) => return Ok(TeamHubRead::Unreachable(unreached)),
      TeamRead::Read { config, unreached } => (config, unreached),
    };
    let db = ProjectDb::open(repo_root)?;
    let readiness = self.readiness_in(&db, repo_root, vault)?;
    let profiles = team_profile::sheets((&db, &config), &config.resources(ResourceKind::AgentProfile))?;
    let programs = match db.sharing()? {
      Some(link) => self.db.lock().mcp_server_programs(link.server)?,
      None => Vec::new(),
    };
    Ok(TeamHubRead::Read(Box::new(TeamHub {
      unreached,
      readiness,
      plain: plain(&config),
      forms: forms(&config),
      mcp_servers: team_mcp::sheets(
        (&db, &config),
        &config.resources(ResourceKind::McpServer),
        &programs,
        &profiles,
      )?,
      tools: team_tool::sheets(&config, &config.resources(ResourceKind::Tool), &profiles),
      instructions: team_text::instructions(&config, &config.resources(ResourceKind::Instruction), &profiles),
      skills: team_text::skills(&config, &config.resources(ResourceKind::Skill), &profiles),
      archived: team_archived::archived((&db, &config), &programs, &profiles)?,
      profiles,
    })))
  }
}
