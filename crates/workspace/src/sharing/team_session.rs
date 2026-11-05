//! The session a team project is read and acted on in (`crate::team_reads::TeamSession`), made from the server the
//! project is linked to: its client with the trust stored with it and this computer's token from the keychain. The
//! session ends when a token of the servers is stored, refused or forgotten, so nothing read or sent in it outlives a
//! sign-out or another account's sign-in.

use orchester_core::id::{ProjectId, ServerId};

use super::{Servers, SharingError};
use crate::team_reads::TeamSession;

impl Servers {
  /// The session of server project `project` on `server`, one of the signed-in account's servers
  /// ([`SharingError::ServerMissing`] otherwise); [`SharingError::SignedOut`] without a token. Blocks on the
  /// application database for the server's entry: call it from a worker of the runtime.
  pub async fn team_session(&self, server: ServerId, project: ProjectId) -> Result<TeamSession, SharingError> {
    self.own(server).await?;
    let entry = self
      .entries()?
      .into_iter()
      .find(|entry| entry.stored.id == server)
      .ok_or(SharingError::ServerMissing)?;
    // Taken before the token: a sign-out while it is read ends the session at once.
    let generation = self.inner.sessions.subscribe();
    let client = self.team_client(&entry)?;
    let token = match self.token(server).await {
      Ok(token) => token,
      Err(SharingError::SignedOut) => {
        self.mark_signed_out(server);
        return Err(SharingError::SignedOut);
      }
      Err(error) => return Err(error),
    };
    Ok(TeamSession::new((client, token), (server, project), generation))
  }
}
