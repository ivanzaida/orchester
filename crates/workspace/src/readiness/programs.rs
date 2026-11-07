//! The program path set on this computer for each team MCP server, as a start takes it.

use std::path::{Path, PathBuf};

use orchester_core::id::ResourceId;
use orchester_store::McpServerProgram;
use serde::{Deserialize, Serialize};

/// The program path of each team MCP server of one team server that was set on this computer, by server resource id.
/// It lives in the application database; a team start reads it once and keeps it with the start, so a start carried on
/// later runs the programs it was started with. A path is not a secret.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpPrograms(Vec<(ResourceId, PathBuf)>);

impl McpPrograms {
  pub(crate) fn of(programs: Vec<McpServerProgram>) -> Self {
    programs
      .into_iter()
      .map(|program| (program.resource, program.program))
      .collect()
  }

  /// The path set for team MCP server `server`; the file may be gone.
  pub fn path(&self, server: ResourceId) -> Option<&Path> {
    self
      .0
      .iter()
      .find(|(resource, _)| *resource == server)
      .map(|(_, path)| path.as_path())
  }

  pub(super) fn iter(&self) -> impl Iterator<Item = (ResourceId, &Path)> {
    self.0.iter().map(|(resource, path)| (*resource, path.as_path()))
  }
}

impl FromIterator<(ResourceId, PathBuf)> for McpPrograms {
  fn from_iter<I: IntoIterator<Item = (ResourceId, PathBuf)>>(programs: I) -> Self {
    Self(programs.into_iter().collect())
  }
}
