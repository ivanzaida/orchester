//! Which team MCP servers are on here.

use std::path::Path;

use orchester_core::{id::ResourceId, resource::ResourceKind, run::Answerer};
use orchester_store::ProjectDb;

use super::{FillInError, team};
use crate::{tasks::outside::Outside, workspace::Workspace};

impl Workspace {
  /// Turns the team MCP server `server` of the project at `repo_root` on or off here. Turning it on lets agents of
  /// the team's profiles use it, so it is a person's alone (`by`); turning it off is always taken, also for a server
  /// that is no longer the team's.
  pub fn switch_team_mcp_on_here(
    &self,
    repo_root: &Path,
    server: ResourceId,
    on: bool,
    by: Answerer,
  ) -> Result<(), FillInError> {
    let mut db = ProjectDb::open(repo_root)?;
    if on {
      team::require_person(by)?;
      team::require_resource(&db, server, ResourceKind::McpServer)?;
      db.set_team_mcp_on_here(server, true)?;
    } else {
      // Kept as a choice: a server switched off is "off here", one nobody decided on needs the person.
      db.set_team_mcp_on_here(server, false)?;
    }
    self.changes.record(db.project().id(), Outside::Connections);
    Ok(())
  }
}
