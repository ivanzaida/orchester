//! A linked project's way into its team server's authority reads (`crate::team_reads`, milestone P6 of
//! `docs/architecture/team-authority-plan.md`): the session they run in, signed in as the person, and who changes the
//! project's team data, as the Members read names them.

use std::sync::Arc;

use orchester_api::MemberRole;
use orchester_core::id::{ProjectId, ServerId};
use tokio::sync::watch;

use super::ProjectServer;
use crate::{
  sharing::{Servers, SharingError},
  team_reads::TeamSession,
};

/// Who changes a team project's tasks, modules and tags, by their username there: its Admins, then the server's admins
/// who are not among them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamAdmins {
  pub admins: Vec<Arc<str>>,
  pub server_admins: Vec<Arc<str>>,
}

impl ProjectServer {
  pub const fn server(&self) -> ServerId {
    self.link.server
  }

  /// The project's id on its server.
  pub const fn team_project(&self) -> ProjectId {
    self.link.project
  }

  /// The session the project's authority reads run in, in the generation `generation` holds: the server's client with
  /// the trust stored with it, and the person's token from the keychain ([`SharingError::SignedOut`] without one).
  pub async fn team_session(&self, generation: watch::Receiver<u64>) -> Result<TeamSession, SharingError> {
    let (client, _) = Servers::client(&self.entry)?;
    let token = self.servers.token(self.link.server).await?;
    Ok(TeamSession::new(
      (client, token),
      (self.link.server, self.link.project),
      generation,
    ))
  }

  /// The project's Admins and the server's admins, as the server lists its members now.
  pub async fn team_admins(&self) -> Result<TeamAdmins, SharingError> {
    let listed = self.members().await?;
    let admins: Vec<_> = listed
      .members
      .iter()
      .filter(|member| member.role == MemberRole::Owner)
      .collect();
    let server_admins = listed
      .admins
      .iter()
      .filter(|admin| !admins.iter().any(|member| member.user.id == admin.user.id))
      .map(|admin| name_of(&admin.username, &admin.user.display_name))
      .collect();
    Ok(TeamAdmins {
      admins: admins
        .iter()
        .map(|member| name_of(&member.username, &member.user.display_name))
        .collect(),
      server_admins,
    })
  }
}

/// A person by their username there, or their name from a server that does not send usernames.
fn name_of(username: &str, display_name: &str) -> Arc<str> {
  Arc::from(if username.is_empty() { display_name } else { username })
}
