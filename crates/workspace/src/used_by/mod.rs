//! What uses a resource, read before it is archived (decision E of `docs/design/team-configuration.md`): archiving is
//! not refused, so the Archive confirmation lists the resources whose latest version names it (*Used by*) and the runs
//! that will stop. One read serves both kinds of project and gives the same shape ([`UsedBy`], the contract of
//! `GET …/resources/{resource}/used-by`, `docs/cloud-api.md`): a team project asks its server, a local project
//! computes the same from its database ([`local`]), by the same rule for "names" ([`names`]) and the same order. The
//! window's modal and automation read it; it only reads.

mod cli;
mod error;
mod local;
mod names;
mod servers;
mod team;

use std::path::Path;

pub use orchester_api::{UsedBy, UsedByResource, UsedByRun};
use orchester_core::id::{ConnectionId, ResourceId};
use orchester_store::{ProjectDb, SharingLink};

pub use self::error::UsedByError;
use crate::{background, team_reads::TeamSession};

/// What uses `resource` in the project at `root`: asked of the server through `team`, the session the person is signed
/// in with, for a project linked to a team server, and computed from the project's database for a local one. A team
/// project read without a session, or in the session of another server project, is refused; one whose server cannot be
/// reached is [`UsedByError::is_not_reached`], never an empty answer. Blocks on the project database: run it on the
/// application's runtime.
pub async fn used_by(root: &Path, team: Option<&TeamSession>, resource: ResourceId) -> Result<UsedBy, UsedByError> {
  let owned = root.to_owned();
  let link = background::run(move || ProjectDb::open(&owned)?.sharing())
    .await
    .map_err(|_| UsedByError::Interrupted)??;
  match link {
    None => {
      let owned = root.to_owned();
      background::run(move || local::used_by(&owned, resource))
        .await
        .map_err(|_| UsedByError::Interrupted)?
    }
    Some(link) => team::used_by(team, &link, resource).await,
  }
}

/// What uses the local MCP server `server` in the project at `root`: the profiles that choose it and the runs that use
/// it. A local MCP server has no team and no versions, so this is read from the project's database alone. Blocks on the
/// project database: run it on the application's runtime.
pub async fn used_by_mcp_server(root: &Path, server: ConnectionId) -> Result<UsedBy, UsedByError> {
  let owned = root.to_owned();
  background::run(move || local::used_by_mcp_server(&owned, server))
    .await
    .map_err(|_| UsedByError::Interrupted)?
}

/// A run an archive stops, as the confirmation lists it: its number among its task's runs and how it stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunLine {
  pub run: orchester_core::id::RunId,
  pub number: u32,
  pub state: RunState,
}

/// How a run that has not ended stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
  Pending,
  Running,
  Paused,
  /// It waits for a person.
  Waiting,
  Interrupted,
  Ended,
}

/// How each of `runs` of the local project at `root` stands. Blocks on the project database.
pub async fn run_lines(root: &Path, runs: Vec<orchester_core::id::RunId>) -> Result<Vec<RunLine>, UsedByError> {
  let owned = root.to_owned();
  background::run(move || local::run_lines(&owned, &runs))
    .await
    .map_err(|_| UsedByError::Interrupted)?
}

/// Whether `session` is the one `link` names.
fn is_of(session: &TeamSession, link: &SharingLink) -> bool {
  session.server() == link.server && session.project() == link.project
}
