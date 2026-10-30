//! Where each listed project is (`docs/design/startup.md`, decision 8), as All projects' *Where* column says it: on
//! this computer only, or shared through a team server, named as the server list names it.

use std::collections::HashMap;

use orchester_core::id::{ProjectId, ServerId};
use orchester_store::ProjectDb;

use super::{Servers, SharingError};
use crate::workspace::ProjectEntry;

/// Where a project is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
  /// Its tasks are on this computer only.
  Local,
  /// Its tasks are a team server's; `team` is the server's name, `None` for a server that is not in the signed-in
  /// account's list.
  Shared { team: Option<String> },
}

/// Where each listed project is. A project is missing when its folder is gone or its data cannot be read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Places(HashMap<ProjectId, Place>);

impl Places {
  pub fn of(&self, project: ProjectId) -> Option<&Place> {
    self.0.get(&project)
  }
}

impl Servers {
  /// Where each project of `entries` is, read from its own data without migrating it. A project whose data cannot be
  /// read is left out and logged: its row says so already. Blocking.
  pub fn places(&self, entries: &[ProjectEntry]) -> Result<Places, SharingError> {
    let names: HashMap<ServerId, String> = self
      .inner
      .workspace
      .servers()?
      .into_iter()
      .map(|server| (server.id, server.name))
      .collect();
    let places = entries
      .iter()
      .filter(|entry| !entry.folder_missing)
      .filter_map(|entry| {
        let root = &entry.project.repo_root;
        match ProjectDb::open_read_only(root).and_then(|db| db.sharing()) {
          Ok(None) => Some((entry.project.id, Place::Local)),
          Ok(Some(link)) => Some((
            entry.project.id,
            Place::Shared {
              team: names.get(&link.server).cloned(),
            },
          )),
          Err(error) => {
            tracing::warn!(%error, root = %root.display(), "cannot read whether a listed project is shared");
            None
          }
        }
      })
      .collect();
    Ok(Places(places))
  }
}
