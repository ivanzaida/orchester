//! Whether each resource of a project is ready on this computer, or exactly what is missing
//! (`docs/design/team-configuration.md`, "Readiness"). One read of the project's database and of the application
//! database gives, for every profile, MCP server and tool the project offers and for every pipeline, core's answer
//! ([`readiness`](orchester_core::sharing::readiness)) to what this computer has: the connections by CLI kind and the
//! per-profile pick, the program path of each team MCP server and whether its file is there now, the secret values set
//! by name, and the team MCP servers switched on.
//!
//! What counts as "has" differs for a team and a local project:
//!
//! - A team secret "has a value here" when `team_secret_values` has a row for its name (a fill-in). The keychain is not
//!   read, so a value removed from it outside Orchester still counts.
//! - A team MCP server is on only when the person switched it on. One they switched off is [`Group::OffHere`]; one
//!   nobody decided on is [`Group::NeedsYouHere`]. Either way it and a profile that chooses it are missing
//!   [`Missing::McpOff`](orchester_core::sharing::Missing::McpOff).
//! - A version of a team profile published here that waits for the server is read from its local body turned into the
//!   team's form as publishing does, so an Admin's own new profile or edit is as ready as it will be when the server
//!   numbers it; one that cannot be turned that way is [`Undecided::CannotBePublished`], with the reason.
//! - In a local project the connection a profile runs on is its pick. The program of each MCP server it chooses that a
//!   run starts must have a program it would start, looked up as the runner does: the `PATH` is the project's
//!   Environment, else Orchester's search path; a program is a file there or in the project (`repo_root` stands for the
//!   worktree), and a fixed file must also lie outside the repository
//!   ([`LocalProgramProblem`](orchester_core::sharing::LocalProgramProblem)). A server turned off in Connections is
//!   left out, as a run leaves it out. Such a server is not a resource of its own, and its variables' secrets are not
//!   read. A tool that sends or connects with a project secret is ready only when the keychain has that secret's value,
//!   which it is asked about without reading the value; a keychain that cannot say is
//!   [`Undecided::SecretUnreadable`].
//! - A tool of a team project names no secret by name until ORC-830, so one that holds none asks nothing. One that
//!   holds a project secret by id is [`Undecided::TeamToolSecretById`]: it is not assumed ready.
//!
//! The read keeps nothing: it is computed from the stores and the filesystem at the call, so a screen or tool that
//! reads it again after a change sees the change. It changes with a fill-in (counted as
//! [`Outside::Connections`](crate::tasks::outside::Outside::Connections) by every write of
//! [`crate::team_fill_ins`]), with a team version the stream cached (`Servers::subscribe_shared`), and with a program
//! file that appears or goes, which nothing announces, so whoever shows the answer reads it again when it is refreshed.
//!
//! Not modelled yet: the account of a connection pick. Instructions, skills and reviewers need nothing of a computer
//! and are not listed.

mod error;
mod facts;
mod local;
mod needs;
mod project;
mod read;
mod standing;
mod team;

use std::path::Path;

use orchester_core::secret::SecretVault;
use orchester_store::ProjectDb;

pub use self::{
  error::ReadinessError,
  project::{PipelineReadiness, ProjectReadiness, ResourceReadiness},
  standing::{Group, ReadinessStanding, Undecided},
};
use crate::workspace::Workspace;

impl Workspace {
  /// Whether each profile, MCP server, tool and pipeline of the project at `repo_root` is ready on this computer. Opens
  /// the project's database (which migrates it, so a caller that only reads uses [`Self::project_readiness_in`] with a
  /// read-only handle), reads the databases, stats the program files and, for the tools of a local project that use a
  /// secret, asks `vault` whether it has the value, so it blocks: run it on the background pool.
  ///
  /// The keychain is asked whether each local tool secret exists and never for its value, so a refresh does not make it
  /// prompt; a caller still reads the answer on an explicit change (a screen opening, a save), not on a timer.
  pub fn project_readiness(
    &self,
    repo_root: &Path,
    vault: &dyn SecretVault,
  ) -> Result<ProjectReadiness, ReadinessError> {
    self.project_readiness_in(&ProjectDb::open(repo_root)?, repo_root, vault)
  }

  /// [`Self::project_readiness`] of the project whose database `db` is already open, e.g. opened only to read it. It
  /// never writes to `db`.
  pub fn project_readiness_in(
    &self,
    db: &ProjectDb,
    repo_root: &Path,
    vault: &dyn SecretVault,
  ) -> Result<ProjectReadiness, ReadinessError> {
    let team_programs = match db.sharing()? {
      Some(link) => Some(self.db.lock().mcp_server_programs(link.server)?),
      None => None,
    };
    read::read(
      db,
      read::Setting {
        repo_root,
        vault,
        team_programs,
      },
    )
  }
}
