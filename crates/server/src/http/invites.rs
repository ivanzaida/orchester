use axum::{
  Json,
  extract::{Query, State},
  http::StatusCode,
};
use orchester_api::{ErrorCode, InviteCreate, InviteCreated, InviteRevoked, Invites};
use orchester_core::id::{InviteId, ProjectId};
use serde::Deserialize;

use super::extract::{Auth, Base, Id, SafeJson};
use crate::{Server, db, error::ServerError, invites};

pub(super) async fn create(
  State(server): State<Server>,
  Auth(auth): Auth,
  Base(base): Base,
  SafeJson(input): SafeJson<InviteCreate>,
) -> Result<(StatusCode, Json<InviteCreated>), ServerError> {
  let state = server.inner.clone();
  let created = server
    .account_write(move |db, now| {
      let current = db::checked(db, &auth, now, false)?;
      invites::create(db, &state.keys, &current.user, &input, &base, now)
    })
    .await?;
  Ok((StatusCode::CREATED, Json(created)))
}

/// `GET /invites?project=<id>&recent=true`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListQuery {
  project: Option<ProjectId>,
  #[serde(default)]
  recent: bool,
}

pub(super) async fn list(
  State(server): State<Server>,
  Auth(auth): Auth,
  query: Result<Query<ListQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<Invites>, ServerError> {
  let Query(query) = query.map_err(|_| ErrorCode::BadRequest)?;
  let listing = invites::Listing {
    project: query.project,
    recent: query.recent,
  };
  let invites = server
    .account_write(move |db, now| {
      let current = db::checked(db, &auth, now, false)?;
      invites::list(db, &current.user, &listing, now)
    })
    .await?;
  Ok(Json(Invites { invites }))
}

pub(super) async fn revoke(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(id): Id<InviteId>,
) -> Result<Json<InviteRevoked>, ServerError> {
  let state = server.inner.clone();
  let revoked = server
    .account_write(move |db, now| {
      let current = db::checked(db, &auth, now, false)?;
      invites::revoke(db, &state.keys, &current.user, id, now)
    })
    .await?;
  Ok(Json(revoked))
}
