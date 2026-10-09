//! Signing in and out, the person's sessions and password, and the admin's users (10.01, 10.04 to
//! 10.08). The token goes from the sign-in answer straight to the keychain; a session signed in with
//! a first password is kept only in memory until the person chose their own password, since it can
//! do nothing else (10.05 C).

use std::{fmt, sync::Arc};

use orchester_api::{
  CreateUser, Credential, PasswordChange, PasswordReset, Session, SignIn, User, UserChanges, UserCreated,
};
use orchester_core::{
  id::{ServerId, SessionId, UserId},
  secret::SecretValue,
  time::Timestamp,
};
use orchester_store::ServerAccount;

use super::{ServerEntry, Servers, SharingError, device};
use crate::background;

/// How signing in ended.
#[derive(Clone, Debug, PartialEq)]
pub enum SignedIn {
  /// The token is in the keychain and the account is recorded.
  Done,
  /// The password was a first password from an admin: the person chooses their own first.
  PasswordChangeRequired(PendingPasswordChange),
}

/// A session signed in with a first password, waiting for the person's own password. Its token is
/// held only here; `Debug` does not show it.
#[derive(Clone)]
pub struct PendingPasswordChange {
  inner: Arc<Pending>,
}

struct Pending {
  server: ServerId,
  token: SecretValue,
  user: User,
}

impl lurq::app::component::DevtoolsInspectable for SignedIn {}

impl PendingPasswordChange {
  pub fn server(&self) -> ServerId {
    self.inner.server
  }
}

impl PartialEq for PendingPasswordChange {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl fmt::Debug for PendingPasswordChange {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("PendingPasswordChange")
      .field("server", &self.inner.server)
      .field("user", &self.inner.user.username)
      .finish_non_exhaustive()
  }
}

impl lurq::app::component::DevtoolsInspectable for PendingPasswordChange {}

fn account(user: &User) -> ServerAccount {
  ServerAccount {
    user: user.id,
    username: user.username.clone(),
    display_name: user.display_name.clone(),
    admin: user.admin,
    signed_in_at: Timestamp::now(),
  }
}

fn token_of(credential: &Credential) -> Result<SecretValue, SharingError> {
  // The server issues `orc_` and 64 hex characters; a blank answer is not a token.
  SecretValue::new(credential.expose().to_owned())
    .map_err(|_| SharingError::Client(orchester_api::client::ClientError::NotOrchester { status: 201 }))
}

impl Servers {
  /// Signs in to `server` with `username` and `password`.
  pub async fn sign_in(
    &self,
    server: &ServerEntry,
    username: String,
    password: Credential,
  ) -> Result<SignedIn, SharingError> {
    let client = Self::client(server)?;
    let request = SignIn {
      username,
      password,
      device: device::this_computer(),
    };
    let answer = client.sign_in(&request).await;
    self.record(server.stored.id, answer.as_ref().err()).await;
    let signed = answer?;
    let token = token_of(&signed.token)?;
    if signed.password_change_required {
      return Ok(SignedIn::PasswordChangeRequired(PendingPasswordChange {
        inner: Arc::new(Pending {
          server: server.stored.id,
          token,
          user: signed.user,
        }),
      }));
    }
    self.keep(server.stored.id, token, &signed.user).await?;
    Ok(SignedIn::Done)
  }

  /// The token goes to the keychain and the account to the list.
  async fn keep(&self, server: ServerId, token: SecretValue, user: &User) -> Result<(), SharingError> {
    self.store_token(server, token).await?;
    let (workspace, account) = (self.inner.workspace.clone(), account(user));
    background::run(move || workspace.set_server_account(server, Some(&account))).await??;
    self.reached(server);
    Ok(())
  }

  /// Chooses the person's own password for a session signed in with a first password; the session
  /// then works and is kept.
  pub async fn choose_password(
    &self,
    server: &ServerEntry,
    pending: &PendingPasswordChange,
    current: Credential,
    new: Credential,
  ) -> Result<(), SharingError> {
    let client = Self::client(server)?;
    let change = PasswordChange {
      current_password: current,
      new_password: new,
    };
    let answer = client.change_password(&pending.inner.token, &change).await;
    self.record(server.stored.id, answer.as_ref().err()).await;
    answer?;
    let token = SecretValue::new(pending.inner.token.expose().to_owned()).map_err(|_| SharingError::SignedOut)?;
    self.keep(server.stored.id, token, &pending.inner.user).await
  }

  /// Signs out of a session that still waits for its own password.
  pub async fn abandon(&self, server: &ServerEntry, pending: &PendingPasswordChange) -> Result<(), SharingError> {
    Self::client(server)?.sign_out(&pending.inner.token).await?;
    Ok(())
  }

  /// Signs this computer out of `server`: the server ends the session if it can be reached, and the
  /// token leaves the keychain and the account the list either way.
  pub async fn sign_out(&self, server: &ServerEntry) -> Result<(), SharingError> {
    let id = server.stored.id;
    match self
      .with_token(server, |client, token| async move { client.sign_out(&token).await })
      .await
    {
      Ok(()) | Err(SharingError::SignedOut) => {}
      // The session ends on the server by itself once unused for 30 days; this computer is
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

  /// Changes the person's password; their other sessions are signed out.
  pub async fn change_password(
    &self,
    server: &ServerEntry,
    current: Credential,
    new: Credential,
  ) -> Result<(), SharingError> {
    let change = PasswordChange {
      current_password: current,
      new_password: new,
    };
    self
      .with_token(server, |client, token| async move {
        client.change_password(&token, &change).await
      })
      .await
  }

  pub async fn users(&self, server: &ServerEntry) -> Result<Vec<User>, SharingError> {
    self
      .with_token(server, |client, token| async move { client.users(&token).await })
      .await
  }

  pub async fn create_user(&self, server: &ServerEntry, user: CreateUser) -> Result<UserCreated, SharingError> {
    self
      .with_token(server, |client, token| async move {
        client.create_user(&token, &user).await
      })
      .await
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

  pub async fn reset_password(&self, server: &ServerEntry, user: UserId) -> Result<PasswordReset, SharingError> {
    self
      .with_token(server, |client, token| async move {
        client.reset_password(&token, user).await
      })
      .await
  }
}
