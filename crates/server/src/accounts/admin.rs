use orchester_api::{CreateUser, Credential, ErrorCode, User, UserChanges, UserCreated};
use orchester_core::id::UserId;

use super::Server;
use crate::{
  db::{self, Authenticated},
  error::ServerError,
  passwords::random_credential,
};

impl Server {
  pub(crate) async fn users(&self, auth: Authenticated) -> Result<Vec<User>, ServerError> {
    let now = self.now();
    self.inner.db.read(move |db| {
      db::checked(db, &auth, now, false, true)?;
      let mut query = db.prepare("SELECT id,username,display_name,admin,disabled_at IS NOT NULL,password_hash,must_change_password FROM users ORDER BY username")?;
      Ok(query.query_map([], |row| Ok(db::user_row(row)?.user))?.collect::<Result<_, _>>()?)
    }).await
  }

  pub(crate) async fn create_user(&self, auth: Authenticated, input: CreateUser) -> Result<UserCreated, ServerError> {
    self.ensure_admin(&auth).await?;
    db::validate_user(&input)?;
    let initial_password = random_credential("", 16)?;
    let hash = self
      .inner
      .passwords
      .hash(Credential::new(initial_password.expose().into()))
      .await?;
    let now = self.now();
    let user = self
      .inner
      .db
      .write(move |tx| {
        let current = db::checked(tx, &auth, now, false, true)?;
        db::create_user(tx, &input, &hash, Some(current.user.id), now)
      })
      .await?;
    Ok(UserCreated { user, initial_password })
  }

  pub(crate) async fn update_user(
    &self,
    auth: Authenticated,
    id: UserId,
    changes: UserChanges,
  ) -> Result<User, ServerError> {
    let now = self.now();
    self
      .inner
      .db
      .write(move |tx| {
        let current = db::checked(tx, &auth, now, false, true)?;
        db::patch_user(tx, id, &changes, current.user.id, now)
      })
      .await
  }

  pub(crate) async fn reset_user(&self, auth: Authenticated, id: UserId) -> Result<Credential, ServerError> {
    self.ensure_admin(&auth).await?;
    let initial = random_credential("", 16)?;
    let hash = self
      .inner
      .passwords
      .hash(Credential::new(initial.expose().into()))
      .await?;
    let now = self.now();
    self
      .inner
      .db
      .write(move |tx| {
        let current = db::checked(tx, &auth, now, false, true)?;
        db::reset_password(tx, id, &hash, Some(current.user.id), now)
      })
      .await?;
    Ok(initial)
  }

  pub(crate) async fn revoke_user_sessions(&self, auth: Authenticated, id: UserId) -> Result<(), ServerError> {
    let now = self.now();
    self
      .inner
      .db
      .write(move |tx| {
        let current = db::checked(tx, &auth, now, false, true)?;
        let exists: bool = tx.query_row(
          "SELECT EXISTS(SELECT 1 FROM users WHERE id=?1)",
          [id.to_string()],
          |row| row.get(0),
        )?;
        if !exists {
          return Err(ErrorCode::NotFound.into());
        }
        db::revoke_sessions(tx, id, None, Some(current.user.id), now)
      })
      .await
  }

  async fn ensure_admin(&self, auth: &Authenticated) -> Result<(), ServerError> {
    let auth = auth.clone();
    let now = self.now();
    self
      .inner
      .db
      .read(move |db| db::checked(db, &auth, now, false, true).map(|_| ()))
      .await
  }
}
