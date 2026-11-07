//! What the Agents hub of a team project lists (`Agents · v2` `acFkz`, ORC-831): the readiness of its profiles, MCP
//! servers and tools ([`crate::readiness`]), and its instructions and skills, which need nothing of a computer and so
//! are always ready here (`docs/design/team-configuration.md`).

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, TeamMcpLaunch, TeamMcpServer, Tool, ToolKind, VersionNumber},
  secret::SecretVault,
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::team_profile::{self, TeamProfileSheet};
use crate::{
  readiness::{ProjectReadiness, ReadinessError},
  workspace::Workspace,
};

/// An instruction or a skill of the team.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plain {
  pub id: ResourceId,
  pub kind: ResourceKind,
  pub name: String,
  /// The latest version the server numbered; `None` while only one published here waits for it.
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
  readiness: ProjectReadiness,
  plain: Vec<Plain>,
  forms: Vec<(ResourceId, Form)>,
  profiles: Vec<(ResourceId, TeamProfileSheet)>,
}

impl TeamHub {
  pub const fn readiness(&self) -> &ProjectReadiness {
    &self.readiness
  }

  /// The team's instructions, then its skills, each by name.
  pub fn plain(&self) -> &[Plain] {
    &self.plain
  }

  /// What the team holds of the profile `resource` and what this computer fills in for it; `None` for any other
  /// resource, and for a profile whose newest version is not in the team's form.
  pub fn profile(&self, resource: ResourceId) -> Option<&TeamProfileSheet> {
    self
      .profiles
      .iter()
      .find_map(|(found, sheet)| (*found == resource).then_some(sheet))
  }

  /// What the MCP server or tool `resource` is; `None` for any other resource.
  pub fn form(&self, resource: ResourceId) -> Option<Form> {
    self
      .forms
      .iter()
      .find_map(|(found, form)| (*found == resource).then_some(*form))
  }
}

/// What each MCP server and tool of the project is, from its latest version.
fn forms(db: &ProjectDb) -> Result<Vec<(ResourceId, Form)>, StoreError> {
  let mut forms = Vec::new();
  for resource in db.resources(ResourceKind::McpServer)? {
    if let Some(version) = db.latest_version::<TeamMcpServer>(resource.id())? {
      let form = match version.body().launch() {
        TeamMcpLaunch::Program(_) => Form::McpProgram,
        TeamMcpLaunch::Remote(_) => Form::McpRemote,
      };
      forms.push((resource.id(), form));
    }
  }
  for resource in db.resources(ResourceKind::Tool)? {
    if let Some(version) = db.latest_version::<Tool>(resource.id())? {
      let form = match version.body().kind() {
        ToolKind::Command => Form::ToolCommand,
        ToolKind::Http => Form::ToolHttp,
        ToolKind::Database => Form::ToolDatabase,
      };
      forms.push((resource.id(), form));
    }
  }
  Ok(forms)
}

impl Workspace {
  /// The hub of the team project at `repo_root`. Reads the databases and the keychain's list of what it has, so it
  /// blocks: run it on the background pool, on an explicit change and never on a timer
  /// ([`Workspace::project_readiness`]).
  pub fn team_hub(&self, repo_root: &Path, vault: &dyn SecretVault) -> Result<TeamHub, ReadinessError> {
    let db = ProjectDb::open(repo_root)?;
    let readiness = self.readiness_in(&db, repo_root, vault)?;
    let standings = db.shared_standings()?;
    let mut plain = Vec::new();
    for kind in [ResourceKind::Instruction, ResourceKind::Skill] {
      let mut of_kind: Vec<Plain> = db
        .resources(kind)?
        .iter()
        .filter_map(|resource| {
          let standing = standings.iter().find(|standing| standing.resource == resource.id())?;
          Some(Plain {
            id: resource.id(),
            kind,
            name: resource.name().to_string(),
            number: standing.confirmed.as_ref().map(|confirmed| confirmed.number),
          })
        })
        .collect();
      of_kind.sort_by_key(|found| found.name.to_lowercase());
      plain.extend(of_kind);
    }
    Ok(TeamHub {
      readiness,
      plain,
      forms: forms(&db)?,
      profiles: team_profile::sheets(&db)?,
    })
  }
}
