//! The program path of a team MCP server on this computer, shared by every project that uses the server.

use std::{fs, path::Path};

use orchester_core::{
  id::{ProjectId, ResourceId, ServerId},
  resource::{ResourceKind, TeamMcpLaunch, TeamMcpServer},
  run::Answerer,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{FillInError, team};
use crate::{tasks::outside::Outside, team_config::TeamResource, workspace::Workspace};

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
    let team_server = team::server_of(&db)?;
    let config = team::config(self, repo_root)?;
    let found = team::require_resource(&config, server, ResourceKind::McpServer)?;
    if runs_remotely(found) {
      return Err(FillInError::RemoteServer(server));
    }
    check_program(program)?;
    self
      .db
      .lock()
      .set_mcp_server_program(team_server, server, program, Timestamp::now())?;
    self.record_program_change(db.project().id(), team_server);
    Ok(())
  }

  /// Forgets the program of the team MCP server `server` of the project at `repo_root`; always taken, also when none
  /// was set or the server is no longer the team's.
  pub fn clear_team_mcp_program(&self, repo_root: &Path, server: ResourceId) -> Result<(), FillInError> {
    let db = ProjectDb::open(repo_root)?;
    let team_server = team::server_of(&db)?;
    self.db.lock().clear_mcp_server_program(team_server, server)?;
    self.record_program_change(db.project().id(), team_server);
    Ok(())
  }

  /// A program path belongs to the team server, not to one project, so every project of this computer shared on
  /// `team_server` shows a change of its connections, whichever account's; `changed`, which made it, always does.
  fn record_program_change(&self, changed: ProjectId, team_server: ServerId) {
    self.changes.record(changed, Outside::Connections);
    let projects = match self.every_project() {
      Ok(projects) => projects,
      Err(error) => {
        tracing::error!(%error, "cannot read the projects that share a program path");
        return;
      }
    };
    for project in projects
      .into_iter()
      .filter(|project| project.id != changed && project.repo_root.is_dir())
    {
      match ProjectDb::open(&project.repo_root).and_then(|other| other.sharing()) {
        Ok(Some(link)) if link.server == team_server => {
          self.changes.record(project.id, Outside::Connections);
        }
        Ok(_) => {}
        // By id: the project may be another account's, whose folders the log does not name.
        Err(error) => tracing::debug!(%error, project = %project.id, "a project's sharing cannot be read"),
      }
    }
  }
}

/// Whether the latest version of `server` the team published is a remote server; one whose latest version cannot be
/// read is not known to be.
fn runs_remotely(server: &TeamResource) -> bool {
  matches!(server.body::<TeamMcpServer>(), Some(Ok(body)) if matches!(body.launch(), TeamMcpLaunch::Remote(_)))
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
