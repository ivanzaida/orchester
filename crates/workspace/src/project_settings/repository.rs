//! Repository checks share setup's detector and refresh rule; checking never silently links a project.
use std::path::Path;

use orchester_core::{
  connection::GitHubConnection,
  repository::{Detection, RepositoryLink},
  store::{ConnectionStore, RepositoryStore},
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  connections::data::github_connections,
  messages::{self, Message},
  repository::{self, DetectTarget},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryRead {
  pub link: Option<RepositoryLink>,
  pub connection: Option<GitHubConnection>,
  pub detection: Option<Detection>,
}
pub fn repository_in(root: &Path) -> Result<RepositoryRead, StoreError> {
  let db = ProjectDb::open(root)?;
  let link = db.repository_link()?;
  let connections = github_connections(db.connections()?);
  let connection = link
    .as_ref()
    .and_then(|link| connections.into_iter().find(|c| c.id() == link.connection()));
  Ok(RepositoryRead {
    link,
    connection,
    detection: None,
  })
}
pub fn check_repository_in(root: &Path) -> Result<RepositoryRead, Message> {
  let mut db = ProjectDb::open(root).map_err(|e| messages::store_error(&e))?;
  let connections = db.connections().map_err(|e| messages::store_error(&e))?;
  let target = DetectTarget {
    repo_root: root.to_path_buf(),
    connections: github_connections(connections),
    program: None,
  };
  let detection = repository::detect(&target);
  // Only a linked project's metadata and existing connection are refreshed.
  if let Some(link) = db.repository_link().map_err(|e| messages::store_error(&e))? {
    let name = target
      .connections
      .iter()
      .find(|c| c.id() == link.connection())
      .map(|c| c.name().as_str())
      .unwrap_or(link.repo().host().as_str());
    repository::refresh(&mut db, &detection, name, orchester_core::time::Timestamp::now()).map_err(|e| e.message())?;
  }
  let mut read = repository_in(root).map_err(|e| messages::store_error(&e))?;
  read.detection = Some(detection);
  Ok(read)
}
