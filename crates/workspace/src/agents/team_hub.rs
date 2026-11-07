//! What the Agents hub of a team project lists (`Agents · v2` `acFkz`, ORC-831): the readiness of its profiles, MCP
//! servers and tools ([`crate::readiness`]), and its instructions and skills, which need nothing of a computer and so
//! are always ready here (`docs/design/team-configuration.md`).

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, VersionNumber},
  secret::SecretVault,
  store::ResourceStore,
};
use orchester_store::ProjectDb;

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

/// The resources the hub lists, and how the profiles, MCP servers and tools stand on this computer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamHub {
  readiness: ProjectReadiness,
  plain: Vec<Plain>,
}

impl TeamHub {
  pub const fn readiness(&self) -> &ProjectReadiness {
    &self.readiness
  }

  /// The team's instructions, then its skills, each by name.
  pub fn plain(&self) -> &[Plain] {
    &self.plain
  }
}

impl Workspace {
  /// The hub of the team project at `repo_root`. Reads the databases and the keychain's list of what it has, so it
  /// blocks: run it on the background pool, on an explicit change and never on a timer
  /// ([`Workspace::project_readiness`]).
  pub fn team_hub(&self, repo_root: &Path, vault: &dyn SecretVault) -> Result<TeamHub, ReadinessError> {
    let readiness = self.project_readiness(repo_root, vault)?;
    let db = ProjectDb::open(repo_root)?;
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
    Ok(TeamHub { readiness, plain })
  }
}
