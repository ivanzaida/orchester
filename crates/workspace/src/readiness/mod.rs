//! Whether each resource of a project is ready on this computer, or exactly what is missing
//! (`docs/design/team-configuration.md`, "Readiness"). One read of the project's database and of the application
//! database gives, for every profile, MCP server and tool the project offers and for every pipeline, core's answer
//! ([`readiness`](orchester_core::sharing::readiness)) to what this computer has: the connections by CLI kind and the
//! per-profile pick, the program path of each team MCP server and whether its file is there now, the secret values set
//! by name, and the team MCP servers switched on.
//!
//! What counts as "has" differs for a team and a local project:
//!
//! - A team secret "has a value here" when `team_secret_values` has a row for its name (a fill-in) and the keychain
//!   still holds its value, which is asked with [`SecretVault::exists`](orchester_core::secret::SecretVault::exists):
//!   the value is not read and the person is not prompted. A row whose value is gone from the keychain is missing
//!   [`Missing::Secret`](orchester_core::sharing::Missing::Secret); a keychain that cannot say is missing
//!   [`Missing::SecretUnreadable`](orchester_core::sharing::Missing::SecretUnreadable), not assumed set.
//! - A team MCP server this app cannot run as defined (a remote one with more than one secret header: an agent's CLI is
//!   given one header per HTTP server) is missing
//!   [`Missing::McpUnsupported`](orchester_core::sharing::Missing::McpUnsupported) in itself and in every profile that
//!   chooses it, instead of passing and being left out of every run.
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
//!   which is read (and dropped) at the call; a keychain that cannot say is [`Undecided::SecretUnreadable`].
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

mod bundle;
mod error;
mod facts;
mod local;
mod needs;
mod not_ready;
mod programs;
mod project;
mod read;
mod standing;
mod team;

use std::path::Path;

use orchester_core::{id::ResourceId, secret::SecretVault};
use orchester_store::ProjectDb;

pub(crate) use self::{bundle::missing as bundle_missing, team::body as team_profile_body};
pub use self::{
  error::ReadinessError,
  not_ready::{MissingPart, NotReadyHere, NotReadyPart, UndecidedPart},
  programs::McpPrograms,
  project::{PipelineReadiness, ProjectReadiness, ResourceReadiness},
  standing::{Group, ReadinessStanding, Undecided},
};
use crate::workspace::Workspace;

/// What a team start needs of this computer before it asks the team server for work.
#[derive(Debug)]
pub enum StartReadiness {
  /// The pipeline is ready here: the program paths this computer has set for the team's MCP servers, which the start
  /// takes with it.
  Ready(McpPrograms),
  NotReady(NotReadyHere),
  /// This computer has no version of the pipeline.
  PipelineUnknown,
}

impl Workspace {
  /// Whether each profile, MCP server, tool and pipeline of the project at `repo_root` is ready on this computer. Reads
  /// the databases, stats the program files and, for the tools of a local project that use a secret, asks `vault`
  /// whether it has the value, so it blocks: run it on the background pool.
  ///
  /// Each local tool secret is read from the keychain on every call, which may prompt on macOS: a caller reads the
  /// answer on an explicit change (a screen opening, a save), not on a timer.
  pub fn project_readiness(
    &self,
    repo_root: &Path,
    vault: &dyn SecretVault,
  ) -> Result<ProjectReadiness, ReadinessError> {
    let db = ProjectDb::open(repo_root)?;
    let team_programs = self.team_programs(&db)?;
    read::read(
      &db,
      read::Setting {
        repo_root,
        vault,
        team_programs,
      },
    )
  }

  /// Whether `pipeline` of the team project at `repo_root` is ready to start on this computer, read before the team
  /// server is asked for anything. It is read from the team versions this computer has cached; the start checks the
  /// versions the team server pins again. Blocks, as [`Self::project_readiness`] does.
  pub fn team_start_readiness(
    &self,
    repo_root: &Path,
    vault: &dyn SecretVault,
    pipeline: ResourceId,
  ) -> Result<StartReadiness, ReadinessError> {
    let db = ProjectDb::open(repo_root)?;
    let programs = self.team_programs(&db)?.ok_or(ReadinessError::NotATeamProject)?;
    let readiness = read::read(
      &db,
      read::Setting {
        repo_root,
        vault,
        team_programs: Some(programs.clone()),
      },
    )?;
    Ok(match readiness.pipeline(pipeline) {
      None => StartReadiness::PipelineUnknown,
      Some(found) => match NotReadyHere::of(&readiness, found) {
        Some(refusal) => StartReadiness::NotReady(refusal),
        None => StartReadiness::Ready(programs),
      },
    })
  }

  /// What keeps a run of `pipeline` of the project at `repo_root` from starting on this computer, as a team start
  /// refuses ([`NotReadyHere`]); `None` when it may start. Reads the project's readiness once
  /// ([`Self::project_readiness`]: it blocks, and it reads the keychain for each local tool secret), so call it as a
  /// start is asked for, never on a timer. A pipeline the project has no published version of is left to the
  /// preflight, which refuses it with its own reason.
  pub fn not_ready_here(
    &self,
    repo_root: &Path,
    vault: &dyn SecretVault,
    pipeline: ResourceId,
  ) -> Result<Option<NotReadyHere>, ReadinessError> {
    let readiness = self.project_readiness(repo_root, vault)?;
    Ok(
      readiness
        .pipeline(pipeline)
        .and_then(|found| NotReadyHere::of(&readiness, found)),
    )
  }

  /// The program paths set for the team MCP servers of the team server `db`'s project is shared with; `None` for a
  /// local project.
  fn team_programs(&self, db: &ProjectDb) -> Result<Option<McpPrograms>, ReadinessError> {
    match db.sharing()? {
      Some(link) => Ok(Some(McpPrograms::of(self.db.lock().mcp_server_programs(link.server)?))),
      None => Ok(None),
    }
  }
}
