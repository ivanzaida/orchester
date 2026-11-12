//! The team's configuration as a shared project's screens show who stands where in it
//! (`docs/design/shared-configuration.md`, decisions 1–5): the team server it is shared on, whether the person may
//! change it (an admin: the project's owner or the server's admin, row 72) and who the admins are. The resources
//! themselves are read from the team server and held in memory (`crate::team_config`); a change of one is sent to the
//! server at once (`crate::sharing::team_edit`), so nothing of the person's waits on this computer.

use std::{path::Path, sync::Arc};

use orchester_api::client::ServerAddress;
use orchester_core::sharing::MemberRole;
use orchester_store::ProjectDb;

use super::{Servers, SharingError, project::ProjectServer};
use crate::messages::{self, Message};

/// A shared project's configuration as its libraries show it; a local project has none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamConfiguration {
  /// The team server's name and host, e.g. *Acme team* and `orchester.acme.dev`; `None` once the server is no longer
  /// in Settings › Orchester.
  pub server: Option<(Arc<str>, Arc<str>)>,
  /// Whether the person may draft, publish, create, import and remove the team's resources.
  pub may_change: bool,
  /// The project's owners by their username there, as the members were last read.
  pub owners: Arc<[Arc<str>]>,
  /// The server's admins who are not among the owners, by their username there, as the members list last named them;
  /// none from a server that does not name them.
  pub admins: Arc<[Arc<str>]>,
  /// The person's own username there, as the members were last read.
  pub me: Option<Arc<str>>,
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
    let owners: Vec<_> = members
      .iter()
      .filter(|member| member.role == MemberRole::Owner)
      .collect();
    let admins = db
      .server_admins()?
      .iter()
      .filter(|admin| !owners.iter().any(|owner| owner.user == admin.user))
      .map(|admin| name_of(admin.username.as_str(), admin.display_name.as_str()))
      .collect();
    let owners = owners
      .iter()
      .map(|member| name_of(member.username.as_str(), member.display_name.as_str()))
      .collect();
    Ok(Some(TeamConfiguration {
      server,
      may_change: db.may_change_configuration()?,
      owners,
      admins,
      me,
    }))
  }
}

/// What to tell the person about `error`, from the server at `address` when it was opened.
pub(crate) fn said(error: &SharingError, address: Option<&ServerAddress>) -> Message {
  match (error, address) {
    (_, Some(address)) => error.message(address),
    (SharingError::Store(error), None) => messages::store_error(error),
    (SharingError::ServerMissing, None) => Message::new("sharing.errors.serverMissing"),
    (SharingError::NotAdmin, None) => Message::new("sharing.errors.notAdmin"),
    (SharingError::LocalEntitiesNamed { names, .. }, None) => {
      Message::new("sharing.errors.localEntitiesNamed").with("names", names.join(", "))
    }
    (SharingError::PipelineNamesNonTeam { name, .. }, None) => {
      Message::new("sharing.errors.pipelineNamesNonTeam").with("name", name.clone())
    }
    (SharingError::WritableFoldersStay { folders, .. }, None) => {
      Message::new("sharing.errors.writableFoldersStay").with("folders", folders.join(", "))
    }
    (_, None) => messages::interrupted(),
  }
}

/// A member as the libraries name them: by their username there, or their name before usernames were kept.
fn name_of(username: &str, display_name: &str) -> Arc<str> {
  Arc::from(if username.is_empty() { display_name } else { username })
}
