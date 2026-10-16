use axum::{
  Json,
  extract::State,
  http::{HeaderMap, StatusCode},
};
use orchester_api::{InviteCreate, InviteCreated, Invites};
use orchester_core::id::InviteId;

use super::{
  base_address,
  extract::{Auth, Id, SafeJson},
};
use crate::{Server, db, error::ServerError, invites};

pub(super) async fn create(
  State(server): State<Server>,
  Auth(auth): Auth,
  headers: HeaderMap,
  SafeJson(input): SafeJson<InviteCreate>,
) -> Result<(StatusCode, Json<InviteCreated>), ServerError> {
  let base = base_address(&server, &headers);
  let state = server.inner.clone();
  let created = server
    .account_write(move |db, now| {
      let current = db::checked(db, &auth, now, false)?;
      invites::create(db, &state.keys, &current.user, &input, &base, now)
    })
    .await?;
  Ok((StatusCode::CREATED, Json(created)))
}

pub(super) async fn list(State(server): State<Server>, Auth(auth): Auth) -> Result<Json<Invites>, ServerError> {
  let invites = server
    .account_write(move |db, now| {
      let current = db::checked(db, &auth, now, false)?;
      invites::list(db, &current.user, now)
    })
    .await?;
  Ok(Json(Invites { invites }))
}

pub(super) async fn revoke(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(id): Id<InviteId>,
) -> Result<StatusCode, ServerError> {
  server
    .account_write(move |db, now| {
      let current = db::checked(db, &auth, now, false)?;
      invites::revoke(db, &current.user, id, now)
    })
    .await?;
  Ok(StatusCode::NO_CONTENT)
}
