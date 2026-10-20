use orchester_api::{Credential, Session};
use orchester_core::id::SessionId;
use rusqlite::params;

use super::Server;
use crate::{
  clock::DEVICE_IDLE_MS,
  db::{self, Authenticated},
  devices::token_hash,
  error::ServerError,
};

impl Server {
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
      let auth = db::checked(db, &auth, now, false)?;
      let mut query = db.prepare("SELECT id,device,created_at,last_used_at,via,mfa FROM sessions WHERE user_id=?1 AND kind='app' AND revoked_at IS NULL AND last_used_at>?2 ORDER BY created_at,id")?;
      let rows = query.query_map(params![auth.user.id.to_string(), now - DEVICE_IDLE_MS], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?, row.get::<_, String>(4)?, row.get::<_, bool>(5)?))
      })?.collect::<Result<Vec<_>, _>>()?;
      rows.into_iter().map(|(id, device, created_at, last_used_at, via, mfa)| {
        let id = id.parse().map_err(|_| ServerError::StoredData)?;
        Ok(Session { id, device, created_at, last_used_at, current: id == auth.session, via: db::parse_via(&via)?, mfa })
      }).collect()
    }).await
  }

  pub(crate) async fn sign_out(&self, auth: Authenticated, session: Option<SessionId>) -> Result<(), ServerError> {
    let now = self.now();
    self
      .inner
      .db
      .write(move |tx| {
        let current = db::checked(tx, &auth, now, false)?;
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
      .read(move |db| Ok(db::checked(db, &auth, now, false)?.user))
      .await
  }
}
