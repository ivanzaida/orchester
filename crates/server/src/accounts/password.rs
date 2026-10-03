use std::net::IpAddr;

use orchester_api::{ErrorCode, PasswordChange};
use orchester_core::id::UserId;
use rusqlite::params;

use super::Server;
use crate::{
  db::{self, Authenticated},
  error::ServerError,
  passwords::PasswordHash,
};

impl Server {
  pub(crate) async fn change_password(
    &self,
    auth: Authenticated,
    input: PasswordChange,
    address: IpAddr,
  ) -> Result<(), ServerError> {
    let user = auth.user.id;
    // Check before any password hashing permit is acquired, including for initial-password sessions.
    if let Some(retry) = self.inner.limits.check_password(user, self.now()) {
      return Err(ServerError::RateLimited(retry));
    }
    let snapshot = auth.clone();
    let now = self.now();
    let hash = self
      .inner
      .db
      .read(move |db| {
        let current = db::checked(db, &snapshot, now, true, false)?;
        Ok(
          db::find_user(db, &current.user.username)?
            .ok_or(ErrorCode::Unauthorized)?
            .hash,
        )
      })
      .await?;
    if !self
      .inner
      .passwords
      .verify(input.current_password, hash.clone())
      .await?
    {
      self.record_password_failure(user, address).await?;
      return Err(ErrorCode::InvalidCredentials.into());
    }
    let new_hash = self.inner.passwords.hash(input.new_password).await?;
    let now = self.now();
    let result = self
      .inner
      .db
      .write(move |tx| change(tx, &auth, &hash, &new_hash, address, now))
      .await;
    if matches!(result, Err(ServerError::Api(ErrorCode::InvalidCredentials))) {
      self.record_password_failure(user, address).await?;
    }
    result
  }

  async fn record_password_failure(&self, user: UserId, address: IpAddr) -> Result<(), ServerError> {
    let now = self.now();
    self.inner.limits.failed_password(user, now);
    self
      .inner
      .db
      .write(move |tx| {
        db::audit(
          tx,
          Some(user),
          "password_change_failed",
          &user.to_string(),
          Some(address),
          now,
        )
      })
      .await
  }
}

fn change(
  tx: &rusqlite::Transaction<'_>,
  auth: &Authenticated,
  previous: &PasswordHash,
  new_hash: &PasswordHash,
  address: IpAddr,
  now: i64,
) -> Result<(), ServerError> {
  let current = db::checked(tx, auth, now, true, false)?;
  let stored = db::find_user(tx, &current.user.username)?.ok_or(ErrorCode::Unauthorized)?;
  if stored.hash.expose() != previous.expose() {
    return Err(ErrorCode::InvalidCredentials.into());
  }
  tx.execute(
    "UPDATE users SET password_hash=?1,must_change_password=0,password_changed_at=?2 WHERE id=?3",
    params![new_hash.expose(), now, current.user.id.to_string()],
  )?;
  tx.execute(
    "UPDATE sessions SET initial_password=0 WHERE id=?1",
    [auth.session.to_string()],
  )?;
  db::revoke_sessions(tx, current.user.id, Some(auth.session), Some(current.user.id), now)?;
  db::audit(
    tx,
    Some(current.user.id),
    "password_changed",
    &current.user.id.to_string(),
    Some(address),
    now,
  )
}
