use axum::{Json, extract::State, http::StatusCode};
use orchester_api::{CreateUser, PasswordReset, UserChanges, UserCreated, UserResponse, UsersResponse};
use orchester_core::id::UserId;

use super::extract::{Auth, Id, SafeJson};
use crate::{Server, error::ServerError};

pub(super) async fn users(State(server): State<Server>, Auth(auth): Auth) -> Result<Json<UsersResponse>, ServerError> {
  Ok(Json(UsersResponse {
    users: server.users(auth).await?,
  }))
}

pub(super) async fn create(
  State(server): State<Server>,
  Auth(auth): Auth,
  SafeJson(input): SafeJson<CreateUser>,
) -> Result<(StatusCode, Json<UserCreated>), ServerError> {
  Ok((StatusCode::CREATED, Json(server.create_user(auth, input).await?)))
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

pub(super) async fn reset(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(id): Id<UserId>,
) -> Result<Json<PasswordReset>, ServerError> {
  Ok(Json(PasswordReset {
    initial_password: server.reset_user(auth, id).await?,
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
