//! The person's account on a team server from this computer (`docs/architecture/server.md`, "Accounts and tokens"):
//! signing out, the account's devices, and the admin's users. Since API 3 nobody has a password on a team server: a
//! device joins with a code or a cloud identity ([`super::joining`]). The password calls below remain only because the
//! frozen `crates/desktop` still calls them; they refuse at once and send nothing.

use std::convert::Infallible;

use orchester_api::{CreateUser, Credential, PasswordReset, Session, User, UserChanges, UserCreated};
use orchester_core::{
  id::{SessionId, UserId},
  time::Timestamp,
};
use orchester_store::ServerAccount;

use super::{ServerEntry, Servers, SharingError};
use crate::background;

/// How signing in with a password ended, for the frozen `crates/desktop`; API 3 servers have no passwords, so it never
/// ends at all.
#[derive(Clone, Debug, PartialEq)]
pub enum SignedIn {
  Done,
  PasswordChangeRequired(PendingPasswordChange),
}

/// A session that waited for its own password; no API 3 server makes one, so none exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingPasswordChange(Infallible);

pub(super) fn account(user: &User) -> ServerAccount {
  ServerAccount {
    user: user.id,
    username: user.username.clone(),
    display_name: user.display_name.clone(),
    admin: user.admin,
    signed_in_at: Timestamp::now(),
  }
}

impl Servers {
  /// Password sign-in, gone with API 3 (row 46): refused at once.
  pub async fn sign_in(
    &self,
    _server: &ServerEntry,
    _username: String,
    _password: Credential,
  ) -> Result<SignedIn, SharingError> {
    Err(SharingError::NoPasswords)
  }

  /// Choosing a first password, gone with API 3: refused at once.
  pub async fn choose_password(
    &self,
    _server: &ServerEntry,
    _pending: &PendingPasswordChange,
    _current: Credential,
    _new: Credential,
  ) -> Result<(), SharingError> {
    Err(SharingError::NoPasswords)
  }

  /// Abandoning a session that waits for its password; none exists since API 3.
  pub async fn abandon(&self, _server: &ServerEntry, _pending: &PendingPasswordChange) -> Result<(), SharingError> {
    Err(SharingError::NoPasswords)
  }

  /// Signs this computer out of `server`, one of the signed-in account's: the server ends the session if it can be
  /// reached, and the token leaves the keychain and the account the list either way.
  pub async fn sign_out(&self, server: &ServerEntry) -> Result<(), SharingError> {
    let id = server.stored.id;
    self.own(id).await?;
    match self
      .with_token(server, |client, token| async move { client.sign_out(&token).await })
      .await
    {
      Ok(()) | Err(SharingError::SignedOut) => {}
      // The device token ends on the server by itself once unused for 90 days; this computer is
      // signed out regardless.
      Err(error) => tracing::warn!(%error, server = %id, "the server did not end the session"),
    }
    self.delete_token(id).await?;
    let workspace = self.inner.workspace.clone();
    background::run(move || workspace.set_server_account(id, None)).await??;
    self.forget(id);
    Ok(())
  }

  pub async fn me(&self, server: &ServerEntry) -> Result<User, SharingError> {
    self
      .with_token(server, |client, token| async move { client.me(&token).await })
      .await
  }

  pub async fn sessions(&self, server: &ServerEntry) -> Result<Vec<Session>, SharingError> {
    self
      .with_token(server, |client, token| async move { client.sessions(&token).await })
      .await
  }

  pub async fn revoke(&self, server: &ServerEntry, session: SessionId) -> Result<(), SharingError> {
    self
      .with_token(server, |client, token| async move {
        client.revoke_session(&token, session).await
      })
      .await
  }

  /// Changing a password, gone with API 3: refused at once.
  pub async fn change_password(
    &self,
    _server: &ServerEntry,
    _current: Credential,
    _new: Credential,
  ) -> Result<(), SharingError> {
    Err(SharingError::NoPasswords)
  }

  pub async fn users(&self, server: &ServerEntry) -> Result<Vec<User>, SharingError> {
    self
      .with_token(server, |client, token| async move { client.users(&token).await })
      .await
  }

  /// Creating a user with a first password, gone with API 3 (accounts come from invites, in the server admin):
  /// refused at once.
  pub async fn create_user(&self, _server: &ServerEntry, _user: CreateUser) -> Result<UserCreated, SharingError> {
    Err(SharingError::NoPasswords)
  }

  /// Disables or enables `user`.
  pub async fn set_disabled(&self, server: &ServerEntry, user: UserId, disabled: bool) -> Result<User, SharingError> {
    let changes = UserChanges {
      disabled: Some(disabled),
      ..UserChanges::default()
    };
    self
      .with_token(server, |client, token| async move {
        client.update_user(&token, user, &changes).await
      })
      .await
  }

  /// Resetting a password, gone with API 3 (an account code replaces it): refused at once.
  pub async fn reset_password(&self, _server: &ServerEntry, _user: UserId) -> Result<PasswordReset, SharingError> {
    Err(SharingError::NoPasswords)
  }
}
