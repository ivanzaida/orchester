use axum::{Json, extract::State, http::StatusCode};
use orchester_api::{AdminBrowsers, AdminSessions, AdminUsers, UserChanges, UserResponse};
use orchester_core::id::{SessionId, UserId};

use super::extract::{Auth, Id, SafeJson};
use crate::{Server, error::ServerError};

pub(super) async fn users(State(server): State<Server>, Auth(auth): Auth) -> Result<Json<AdminUsers>, ServerError> {
  Ok(Json(AdminUsers {
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

pub(super) async fn sessions(
  State(server): State<Server>,
  Auth(auth): Auth,
) -> Result<Json<AdminSessions>, ServerError> {
  Ok(Json(AdminSessions {
    sessions: server.admin_sessions(auth).await?,
  }))
}

pub(super) async fn revoke_session(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(id): Id<SessionId>,
) -> Result<StatusCode, ServerError> {
  server.revoke_admin_session(auth, id).await?;
  Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn browsers(
  State(server): State<Server>,
  Auth(auth): Auth,
) -> Result<Json<AdminBrowsers>, ServerError> {
  Ok(Json(AdminBrowsers {
    browsers: server.admin_browsers(auth).await?,
  }))
}

pub(super) async fn revoke_browser(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(id): Id<SessionId>,
) -> Result<StatusCode, ServerError> {
  server.revoke_admin_browser(auth, id).await?;
  Ok(StatusCode::NO_CONTENT)
}
