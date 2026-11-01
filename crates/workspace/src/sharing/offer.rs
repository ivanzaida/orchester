//! The team's project All projects offers above its list (`docs/design/startup.md`, decision 8): one the person is a
//! member of on a joined team server and has no checkout of on this computer yet, until the person answers *Not now*
//! for it. What was put off is kept across starts in the application database, per server and project.

use std::collections::HashSet;

use orchester_api::SharedProject;
use orchester_core::{
  id::{ProjectId, ServerId},
  time::Timestamp,
};
use orchester_store::StoreError;

use super::{Servers, SharingError};
use crate::{background, workspace::Workspace};

/// The projects put off, each under `<server>.<project>`.
const NOT_NOW: &str = "projects.offer.not-now.";

/// A project on a joined team server with no checkout on this computer.
#[derive(Clone, Debug)]
pub struct OfferedProject {
  pub server: ServerId,
  /// The server's name, as the list names it.
  pub team: String,
  pub project: SharedProject,
}

impl PartialEq for OfferedProject {
  fn eq(&self, other: &Self) -> bool {
    (self.server, self.project.id) == (other.server, other.project.id)
  }
}

impl Servers {
  /// The first project with no checkout here that was not put off, on the signed-in account's servers in use, in the
  /// order they were added; `None` when there is none.
  pub async fn offered(&self) -> Result<Option<OfferedProject>, SharingError> {
    let servers = self.clone();
    let entries = background::run(move || servers.entries()).await??;
    let workspace = self.inner.workspace.clone();
    let put_off = background::run(move || put_off(&workspace)).await?;
    for entry in entries.iter().filter(|entry| entry.in_use()) {
      let projects = match self.server_projects(entry).await {
        Ok(projects) => projects,
        Err(error) => {
          // A server that cannot be read now offers nothing: its own page in Settings › Orchester says why, and the
          // offer comes when it answers again.
          tracing::debug!(%error, server = %entry.stored.id, "cannot read a team's projects to offer one");
          continue;
        }
      };
      let server = entry.stored.id;
      if let Some(listed) = projects
        .into_iter()
        .find(|listed| listed.checkout.is_none() && !put_off.contains(&(server, listed.project.id)))
      {
        return Ok(Some(OfferedProject {
          server,
          team: entry.stored.name.clone(),
          project: listed.project,
        }));
      }
    }
    Ok(None)
  }

  /// The person answered *Not now* for `project` on `server`: it is not offered again. Blocking.
  pub fn not_now(&self, server: ServerId, project: ProjectId) -> Result<(), StoreError> {
    self.inner.workspace.set_ui_state(
      &format!("{NOT_NOW}{server}.{project}"),
      &Timestamp::now().unix_millis().to_string(),
    )
  }
}

/// The projects put off so far. When they cannot be read, that is logged and each one is offered again.
fn put_off(workspace: &Workspace) -> HashSet<(ServerId, ProjectId)> {
  let keys = match workspace.ui_state_keys(NOT_NOW) {
    Ok(keys) => keys,
    Err(error) => {
      tracing::warn!(%error, "cannot read the team projects put off");
      return HashSet::new();
    }
  };
  keys
    .iter()
    .filter_map(|key| {
      // A key not written by `not_now` puts nothing off.
      let (server, project) = key.strip_prefix(NOT_NOW)?.split_once('.')?;
      Some((server.parse().ok()?, project.parse().ok()?))
    })
    .collect()
}
