//! The program path of a team MCP server on this computer, shared by every project that uses the server.

use std::{fs, path::Path};

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, TeamMcpLaunch, TeamMcpServer},
  run::Answerer,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{FillInError, team};
use crate::{tasks::outside::Outside, workspace::Workspace};

impl Workspace {
  /// Sets `program`, the person's choice from *Choose…*, as the program of the team MCP server `server` of the
  /// project at `repo_root` on this computer. Nothing is guessed or found by name: the path must be absolute and a
  /// file, and the server must run a local program. It lets agents run that file, so it is a person's alone (`by`).
  pub fn set_team_mcp_program(
    &self,
    repo_root: &Path,
    server: ResourceId,
    program: &Path,
    by: Answerer,
  ) -> Result<(), FillInError> {
    team::require_person(by)?;
    let db = ProjectDb::open(repo_root)?;
    team::require_resource(&db, server, ResourceKind::McpServer)?;
    let team_server = team::server_of(&db)?;
    if runs_remotely(&db, server)? {
      return Err(FillInError::RemoteServer(server));
    }
    check_program(program)?;
    self
      .db
      .lock()
      .set_mcp_server_program(team_server, server, program, Timestamp::now())?;
    self.changes.record(db.project().id(), Outside::Connections);
    Ok(())
  }

  /// Forgets the program of the team MCP server `server` of the project at `repo_root`; always taken, also when none
  /// was set or the server is no longer the team's.
  pub fn clear_team_mcp_program(&self, repo_root: &Path, server: ResourceId) -> Result<(), FillInError> {
    let db = ProjectDb::open(repo_root)?;
    let team_server = team::server_of(&db)?;
    self.db.lock().clear_mcp_server_program(team_server, server)?;
    self.changes.record(db.project().id(), Outside::Connections);
    Ok(())
  }
}

/// Whether the newest version of `server` the team published is a remote server; one with no version cached yet is
/// not known to be.
fn runs_remotely(db: &ProjectDb, server: ResourceId) -> Result<bool, FillInError> {
  Ok(
    db.latest_version::<TeamMcpServer>(server)?
      .is_some_and(|version| matches!(version.body().launch(), TeamMcpLaunch::Remote(_))),
  )
}

fn check_program(program: &Path) -> Result<(), FillInError> {
  if !program.is_absolute() {
    return Err(FillInError::ProgramNotAbsolute);
  }
  if fs::metadata(program).is_ok_and(|found| found.is_file()) {
    Ok(())
  } else {
    Err(FillInError::ProgramNotAFile)
  }
}
