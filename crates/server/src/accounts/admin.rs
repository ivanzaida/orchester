use orchester_api::{ErrorCode, User, UserChanges};
use orchester_core::id::UserId;

use super::Server;
use crate::{
  db::{self, Authenticated},
  error::ServerError,
};

impl Server {
  pub(crate) async fn users(&self, auth: Authenticated) -> Result<Vec<User>, ServerError> {
    let now = self.now();
    self
      .inner
      .db
      .read(move |db| {
        db::checked(db, &auth, now, true)?;
        let mut query = db.prepare(&format!("SELECT {} FROM users ORDER BY username", db::USER_COLUMNS))?;
        Ok(query.query_map([], db::user_row)?.collect::<Result<_, _>>()?)
      })
      .await
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
        let current = db::checked(tx, &auth, now, true)?;
        db::patch_user(tx, id, &changes, current.user.id, now)
      })
      .await
  }

  pub(crate) async fn revoke_user_sessions(&self, auth: Authenticated, id: UserId) -> Result<(), ServerError> {
    let now = self.now();
    self
      .inner
      .db
      .write(move |tx| {
        let current = db::checked(tx, &auth, now, true)?;
        if db::user_by_id(tx, id)?.is_none() {
          return Err(ErrorCode::NotFound.into());
        }
        db::revoke_sessions(tx, id, None, Some(current.user.id), now)
      })
      .await
  }
}
