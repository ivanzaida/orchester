//! Project settings › Sharing › *Link to a project* for a local project (`docs/architecture/server.md`, rows 49–50):
//! the project chosen among the person's projects on a connected server, or by its address (typed, or prefilled from
//! `orchester.toml` and contacted only after the person confirms it), and the checkout's `origin` compared with the
//! project's repository hint. A mismatch warns and the person may go on; the rule is [`remote_match`].

use std::path::Path;

use orchester_api::{SharedProject, client::ServerAddress};
use orchester_core::repository::RepositoryIdentity;
use orchester_exec::programs::Programs;

use super::share::repository_identity;
use crate::sharing::{ProjectAddress, RemoteMatch, ServerEntry, Servers, SharingError, remote_match};

/// A server project this checkout could be linked to, and how its `origin` compares with the project's hint.
#[derive(Clone, Debug)]
pub struct LinkTarget {
  pub entry: ServerEntry,
  pub project: SharedProject,
  pub remote: RemoteMatch,
}

/// Why an address leads to no project this checkout can be linked to.
#[derive(Debug, thiserror::Error)]
pub enum LinkProblem {
  /// The address is on a server this computer is not connected to; nothing was sent to it.
  #[error("not connected to the server at {0:?}")]
  NotConnected(ServerAddress),
  #[error(transparent)]
  Sharing(#[from] SharingError),
}

/// The `origin` of the checkout at `repo_root`, `None` without a network one. Blocking.
pub fn checkout_origin(programs: &Programs, repo_root: &Path) -> Option<RepositoryIdentity> {
  repository_identity(programs, repo_root).ok()
}

impl Servers {
  /// The project at `address` for the checkout whose `origin` is `origin`: on a server this computer is connected to,
  /// and one the person is a member of ([`SharingError::NoProject`] otherwise: the server answers both alike).
  pub async fn link_target(
    &self,
    address: &ProjectAddress,
    origin: Option<&RepositoryIdentity>,
  ) -> Result<LinkTarget, LinkProblem> {
    let servers = self.clone();
    let entries = crate::background::run(move || servers.entries())
      .await
      .map_err(SharingError::from)??;
    let entry = entries
      .into_iter()
      .find(|entry| entry.in_use() && entry.address == *address.server())
      .ok_or_else(|| LinkProblem::NotConnected(address.server().clone()))?;
    let project = self.find_project(&entry, address.slug()).await?;
    Ok(Self::target(entry, project, origin))
  }

  /// `project` on `entry` as a target for the checkout whose `origin` is `origin`.
  pub fn target(entry: ServerEntry, project: SharedProject, origin: Option<&RepositoryIdentity>) -> LinkTarget {
    let remote = remote_match(origin, project.remote_hint.as_ref());
    LinkTarget { entry, project, remote }
  }
}
