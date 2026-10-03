use std::net::IpAddr;

use orchester_api::{Credential, ErrorCode, Session, SignIn, SignedIn};
use orchester_core::id::SessionId;
use rusqlite::params;

use super::Server;
use crate::{
  clock::SESSION_IDLE_MS,
  db::{self, Authenticated},
  error::ServerError,
  passwords::{PasswordHash, random_credential, token_hash},
};

impl Server {
  pub(crate) async fn sign_in(&self, input: SignIn, address: IpAddr) -> Result<SignedIn, ServerError> {
    let username = input.username.to_ascii_lowercase();
    if username.is_empty()
      || username.len() > 64
      || !username
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
      return Err(ErrorCode::InvalidCredentials.into());
    }
    if input.device.trim().is_empty() || input.device.chars().count() > 256 {
      return Err(ErrorCode::Invalid.into());
    }
    if let Some(retry) = self.inner.limits.check(&username, address, self.now()) {
      return Err(ServerError::RateLimited(retry));
    }
    let lookup = username.clone();
    let stored = self.inner.db.read(move |db| db::find_user(db, &lookup)).await?;
    let hash = stored
      .as_ref()
      .map_or_else(PasswordHash::dummy, |user| user.hash.clone());
    let verified = self.inner.passwords.verify(input.password, hash.clone()).await?;
    let now = self.now();
    // Requests queued behind password hashing also observe a limit reached in the meantime.
    if let Some(retry) = self.inner.limits.check(&username, address, now) {
      return Err(ServerError::RateLimited(retry));
    }
    if !verified || stored.as_ref().is_none_or(|user| user.user.disabled) {
      self.record_sign_in_failure(username, address, now).await?;
      return Err(ErrorCode::InvalidCredentials.into());
    }
    let token = random_credential("orc_", 32)?;
    let digest = token_hash(&token);
    let session = Session {
      id: SessionId::generate(),
      device: input.device,
      created_at: now,
      last_used_at: now,
      current: true,
    };
    let failure_name = username.clone();
    let result = self.inner.db.write(move |tx| {
      let current = db::find_user(tx, &username)?.ok_or(ErrorCode::InvalidCredentials)?;
      if current.user.disabled || current.hash.expose() != hash.expose() { return Err(ErrorCode::InvalidCredentials.into()); }
      tx.execute("INSERT INTO sessions(id,user_id,token_hash,device,created_at,last_used_at,initial_password) VALUES (?1,?2,?3,?4,?5,?5,?6)",
        params![session.id.to_string(), current.user.id.to_string(), digest, session.device, now, current.must_change])?;
      db::audit(tx, Some(current.user.id), "signed_in", &session.id.to_string(), Some(address), now)?;
      Ok((current.user, current.must_change, session))
    }).await;
    let (user, must_change, session) = match result {
      Err(ServerError::Api(ErrorCode::InvalidCredentials)) => {
        self.record_sign_in_failure(failure_name, address, now).await?;
        return Err(ErrorCode::InvalidCredentials.into());
      }
      other => other?,
    };
    Ok(SignedIn {
      token,
      session,
      user,
      password_change_required: must_change,
    })
  }

  async fn record_sign_in_failure(&self, username: String, address: IpAddr, now: i64) -> Result<(), ServerError> {
    self.inner.limits.failed(username.clone(), address, now);
    self
      .inner
      .db
      .write(move |tx| db::audit(tx, None, "sign_in_failed", &username, Some(address), now))
      .await
  }

  pub(crate) async fn authenticate(&self, token: Credential) -> Result<Authenticated, ServerError> {
    let hash = token_hash(&token);
    let now = self.now();
    let auth = self.inner.db.read(move |db| db::authenticate(db, &hash, now)).await?;
    if now - auth.last_used_at <= 60_000 {
      return Ok(auth);
    }
    self.inner.db.write(move |tx| db::touch_session(tx, &auth, now)).await
  }

  pub(crate) async fn sessions(&self, auth: Authenticated) -> Result<Vec<Session>, ServerError> {
    let now = self.now();
    self.inner.db.read(move |db| {
      let auth = db::checked(db, &auth, now, false, false)?;
      let mut query = db.prepare("SELECT id,device,created_at,last_used_at FROM sessions WHERE user_id=?1 AND revoked_at IS NULL AND last_used_at>?2 ORDER BY created_at,id")?;
      let rows = query.query_map(params![auth.user.id.to_string(), now - SESSION_IDLE_MS], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?))
      })?.collect::<Result<Vec<_>, _>>()?;
      rows.into_iter().map(|(id, device, created_at, last_used_at)| {
        let id = id.parse().map_err(|_| ServerError::StoredData)?;
        Ok(Session { id, device, created_at, last_used_at, current: id == auth.session })
      }).collect()
    }).await
  }

  pub(crate) async fn sign_out(&self, auth: Authenticated, session: Option<SessionId>) -> Result<(), ServerError> {
    let now = self.now();
    self
      .inner
      .db
      .write(move |tx| {
        let current = db::checked(tx, &auth, now, session.is_none(), false)?;
        db::revoke_session(
          tx,
          current.user.id,
          session.unwrap_or(auth.session),
          current.user.id,
          now,
        )
      })
      .await
  }

  pub(crate) async fn me(&self, auth: Authenticated) -> Result<orchester_api::User, ServerError> {
    let now = self.now();
    self
      .inner
      .db
      .read(move |db| Ok(db::checked(db, &auth, now, false, false)?.user))
      .await
  }
}
