//! A team server's projects as Settings › Orchester lists them under its entry (`docs/architecture/server.md`, rows
//! 49–50): the local projects on this computer linked to a project there, which signing out names, and the person's
//! projects on the server with the checkout each is linked to here, if any.

use std::path::PathBuf;

use orchester_api::SharedProject;
use orchester_core::{id::ServerId, name::Name};
use orchester_store::{ProjectDb, SharingLink};

use super::{ServerEntry, Servers, SharingError, trust::Trust};
use crate::background;

/// A local project linked to a project on a team server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedProject {
  pub name: Name,
  pub repo_root: PathBuf,
  pub link: SharingLink,
}

/// One of the person's projects on a server, and the local checkout linked to it on this computer.
#[derive(Clone, Debug)]
pub struct ServerProject {
  pub project: SharedProject,
  pub checkout: Option<LinkedProject>,
}

impl Servers {
  /// The local projects linked to a project on `server`, in the order they were added. A project whose folder cannot
  /// be read is left out and logged. Blocking.
  pub fn linked_projects(&self, server: ServerId) -> Result<Vec<LinkedProject>, SharingError> {
    let registered = self.inner.workspace.projects()?;
    let mut linked = Vec::new();
    for project in registered {
      let link = match ProjectDb::open(&project.repo_root).and_then(|db| db.sharing()) {
        Ok(link) => link,
        Err(error) => {
          tracing::warn!(%error, root = %project.repo_root.display(), "cannot read whether a project is shared");
          continue;
        }
      };
      if let Some(link) = link.filter(|link| link.server == server) {
        linked.push(LinkedProject {
          name: project.name,
          repo_root: project.repo_root,
          link,
        });
      }
    }
    Ok(linked)
  }

  /// The person's projects on `entry`, each with the local checkout linked to it.
  pub async fn server_projects(&self, entry: &ServerEntry) -> Result<Vec<ServerProject>, SharingError> {
    let projects = self
      .with_token(entry, |client, token| async move { client.projects(&token).await })
      .await?;
    let (servers, server) = (self.clone(), entry.stored.id);
    let linked = background::run(move || servers.linked_projects(server)).await??;
    Ok(
      projects
        .into_iter()
        .map(|project| ServerProject {
          checkout: linked.iter().find(|linked| linked.link.project == project.id).cloned(),
          project,
        })
        .collect(),
    )
  }

  /// Trusts the certificate the person confirmed for `entry` after it presented another one (*It matches · trust
  /// it*), and asks the server again now.
  pub async fn trust_again(&self, entry: &ServerEntry, trust: Trust) -> Result<(), SharingError> {
    let (pin, plain_http) = trust.columns();
    let (workspace, server) = (self.inner.workspace.clone(), entry.stored.id);
    background::run(move || workspace.set_server_trust(server, pin.as_deref(), plain_http)).await??;
    self.forget(server);
    self.retry_now(server);
    Ok(())
  }
}
