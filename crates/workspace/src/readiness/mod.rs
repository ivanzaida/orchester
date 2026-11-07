//! Whether each resource of a project is ready on this computer, or exactly what is missing
//! (`docs/design/team-configuration.md`, "Readiness"). One read of the project's database and of the application
//! database gives, for every profile, MCP server and tool the project offers and for every pipeline, core's answer
//! ([`readiness`](orchester_core::sharing::readiness)) to what this computer has: the connections by CLI kind and the
//! per-profile pick, the program path of each team MCP server and whether its file is there now, the secret values set
//! by name, and the team MCP servers switched on. A local project is read the same way: the connection a profile runs
//! on is its pick, and an MCP server of the project has no readiness of its own (it is on, and the profile's
//! connection is all its profile needs).
//!
//! The read keeps nothing: it is computed from the stores and the filesystem at the call, so a screen or tool that
//! reads it again after a change sees the change. It changes with a fill-in (counted as
//! [`Outside::Connections`](crate::tasks::outside::Outside::Connections) by every write of
//! [`crate::team_fill_ins`]), with a team version the stream cached (`Servers::subscribe_shared`), and with a program
//! file that appears or goes, which nothing announces, so whoever shows the answer reads it again when it is refreshed.
//!
//! Not modelled yet: the secrets of a tool (a tool body names them by id, not by name, until ORC-830), and the account
//! of a connection pick. Instructions, skills and reviewers need nothing of a computer and are not listed.

mod error;
mod facts;
mod needs;
mod project;
mod read;
mod standing;

use std::path::Path;

use orchester_store::ProjectDb;

pub use self::{
  error::ReadinessError,
  project::{PipelineReadiness, ProjectReadiness, ResourceReadiness},
  standing::{Group, Standing, Undecided},
};
use crate::workspace::Workspace;

impl Workspace {
  /// Whether each profile, MCP server, tool and pipeline of the project at `repo_root` is ready on this computer. Reads
  /// the databases and stats the program files, so it blocks: run it on the background pool.
  pub fn project_readiness(&self, repo_root: &Path) -> Result<ProjectReadiness, ReadinessError> {
    let db = ProjectDb::open(repo_root)?;
    let programs = match db.sharing()? {
      Some(link) => self.db.lock().mcp_server_programs(link.server)?,
      None => Vec::new(),
    };
    read::read(&db, programs)
  }
}
