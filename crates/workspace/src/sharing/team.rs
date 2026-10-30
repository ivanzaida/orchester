//! The team's configuration as a shared project's five libraries and their editors show it
//! (`docs/design/shared-configuration.md`, decisions 1–5): the team server it is shared on, whether the person may
//! change it (an admin: the project's owner or the server's admin, row 72) and who the admins are, and how each of the
//! team's resources stands here. And what an admin's editor does with a version published here: send it now (*Publish
//! for the team*, *Try again*), or publish it anyway as the next version after one someone published meanwhile.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use orchester_api::client::ServerAddress;
use orchester_core::{
  id::{ResourceId, VersionId},
  sharing::MemberRole,
};
use orchester_store::{ProjectDb, SharedStanding};

use super::{Servers, SharingError, project::ProjectServer, sync::RemoteResourceStore};
use crate::{
  background,
  messages::{self, Message},
};

/// A shared project's configuration as its libraries show it; a local project has none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamConfiguration {
  /// The team server's name and host, e.g. *Acme team* and `orchester.acme.dev`; `None` once the server is no longer
  /// in Settings › Orchester.
  pub server: Option<(Arc<str>, Arc<str>)>,
  /// Whether the person may draft, publish, create, import and remove the team's resources.
  pub may_change: bool,
  /// The project's owners by their username there, as the members were last read; the server's admins are not listed
  /// to a member.
  pub owners: Arc<[Arc<str>]>,
  /// The person's own username there, as the members were last read: who published a version that waits here.
  pub me: Option<Arc<str>>,
  /// How each of the team's resources stands here.
  pub standings: Arc<BTreeMap<ResourceId, SharedStanding>>,
}

impl TeamConfiguration {
  /// How `resource` stands; `None` for a resource that is not the team's.
  pub fn standing(&self, resource: ResourceId) -> Option<&SharedStanding> {
    self.standings.get(&resource)
  }
}

impl Servers {
  /// The configuration of the project at `repo_root` as its libraries show it; `None` while it is local. Blocking.
  pub fn team_configuration(&self, repo_root: &Path) -> Result<Option<TeamConfiguration>, SharingError> {
    // Opening the server project records whether the person is the server's admin, which `may_change` reads.
    let server = match ProjectServer::open(self, repo_root) {
      Ok(server) => server.map(|server| {
        let entry = server.entry();
        (Arc::from(entry.stored.name.as_str()), Arc::from(entry.address.host()))
      }),
      Err(SharingError::ServerMissing) => None,
      Err(error) => return Err(error),
    };
    let db = ProjectDb::open(repo_root)?;
    if db.sharing()?.is_none() {
      return Ok(None);
    }
    let read_as = db.members_read_as()?;
    // Members never read yet name nobody: the line then names the admins by their roles.
    let members = db.shared_members()?.map(|(_, members)| members).unwrap_or_default();
    let me = members
      .iter()
      .find(|member| Some(member.user) == read_as)
      .map(|member| name_of(member.username.as_str(), member.display_name.as_str()));
    let owners = members
      .iter()
      .filter(|member| member.role == MemberRole::Owner)
      .map(|member| name_of(member.username.as_str(), member.display_name.as_str()))
      .collect();
    let standings = db
      .shared_standings()?
      .into_iter()
      .map(|standing| (standing.resource, standing))
      .collect();
    Ok(Some(TeamConfiguration {
      server,
      may_change: db.may_change_configuration()?,
      owners,
      me,
      standings: Arc::new(standings),
    }))
  }

  /// Sends the versions of the team's configuration published here at `repo_root` that wait, now: after *Publish for
  /// the team* and on *Try again*; a local project has none. A version the server cannot take yet waits, and one that
  /// met a version someone published from the same base waits for the admin's decision; both show on its editor, so
  /// neither fails here. Fails with what to tell the person.
  pub async fn send_published(&self, repo_root: &Path) -> Result<(), Message> {
    self.sending(repo_root, |store| store.send_waiting()).await
  }

  /// *Publish mine as vN*: `version`, published here at `repo_root` and met by a version someone published from the
  /// same base, is sent again as the next version after it. Fails with what to tell the person.
  pub async fn publish_again(&self, repo_root: &Path, version: VersionId) -> Result<(), Message> {
    self.sending(repo_root, move |store| store.publish_again(version)).await
  }

  /// Runs `send` with the store of the project at `repo_root` on a worker; nothing for a local project.
  async fn sending(
    &self,
    repo_root: &Path,
    send: impl FnOnce(&mut RemoteResourceStore<ProjectServer>) -> Result<(), SharingError> + Send + 'static,
  ) -> Result<(), Message> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let sent = background::run(move || {
      let server = match ProjectServer::open(&servers, &root) {
        Ok(Some(server)) => server,
        Ok(None) => return Ok(()),
        Err(error) => return Err(said(&error, None)),
      };
      let address = server.entry().address.clone();
      let store = ProjectDb::open(&root)
        .map_err(SharingError::from)
        .and_then(|db| RemoteResourceStore::new(db, server));
      match store.and_then(|mut store| send(&mut store)) {
        Ok(()) | Err(SharingError::Offline | SharingError::VersionConflict { .. }) => Ok(()),
        Err(error) => Err(said(&error, Some(&address))),
      }
    })
    .await?;
    self.shared_changed();
    sent
  }
}

/// What to tell the person about `error`, from the server at `address` when it was opened.
fn said(error: &SharingError, address: Option<&ServerAddress>) -> Message {
  match (error, address) {
    (_, Some(address)) => error.message(address),
    (SharingError::Store(error), None) => messages::store_error(error),
    (SharingError::ServerMissing, None) => Message::new("sharing.errors.serverMissing"),
    (SharingError::NotAdmin, None) => Message::new("sharing.errors.notAdmin"),
    (_, None) => messages::interrupted(),
  }
}

/// A member as the libraries name them: by their username there, or their name before usernames were kept.
fn name_of(username: &str, display_name: &str) -> Arc<str> {
  Arc::from(if username.is_empty() { display_name } else { username })
}
