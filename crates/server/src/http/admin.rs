use axum::{Json, extract::State, http::StatusCode};
use orchester_api::{UserChanges, UserResponse, UsersResponse};
use orchester_core::id::UserId;

use super::extract::{Auth, Id, SafeJson};
use crate::{Server, error::ServerError};

pub(super) async fn users(State(server): State<Server>, Auth(auth): Auth) -> Result<Json<UsersResponse>, ServerError> {
  Ok(Json(UsersResponse {
    users: server.users(auth).await?,
  }))
}

pub(super) async fn update(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(id): Id<UserId>,
  SafeJson(changes): SafeJson<UserChanges>,
) -> Result<Json<UserResponse>, ServerError> {
  Ok(Json(UserResponse {
    user: server.update_user(auth, id, changes).await?,
  }))
}

pub(super) async fn revoke(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(id): Id<UserId>,
) -> Result<StatusCode, ServerError> {
  server.revoke_user_sessions(auth, id).await?;
  Ok(StatusCode::NO_CONTENT)
}
