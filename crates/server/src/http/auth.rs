use axum::{
  Json,
  extract::{ConnectInfo, State},
  http::{HeaderMap, StatusCode},
  response::{IntoResponse, Response},
};
use orchester_api::{PasswordChange, SessionsResponse, SignIn, UserResponse};
use orchester_core::id::SessionId;

use super::{
  address,
  extract::{Auth, Id, SafeJson},
};
use crate::{Server, error::ServerError, tls::PeerAddr};

pub(super) async fn sign_in(
  State(server): State<Server>,
  peer: ConnectInfo<PeerAddr>,
  headers: HeaderMap,
  SafeJson(input): SafeJson<SignIn>,
) -> Result<Response, ServerError> {
  let signed_in = server.sign_in(input, address(&server, peer, &headers)).await?;
  let id = signed_in.user.id;
  let mut response = (StatusCode::CREATED, Json(signed_in)).into_response();
  response.extensions_mut().insert(id);
  Ok(response)
}

pub(super) async fn me(State(server): State<Server>, Auth(auth): Auth) -> Result<Json<UserResponse>, ServerError> {
  Ok(Json(UserResponse {
    user: server.me(auth).await?,
  }))
}

pub(super) async fn sessions(
  State(server): State<Server>,
  Auth(auth): Auth,
) -> Result<Json<SessionsResponse>, ServerError> {
  Ok(Json(SessionsResponse {
    sessions: server.sessions(auth).await?,
  }))
}

pub(super) async fn sign_out(State(server): State<Server>, Auth(auth): Auth) -> Result<StatusCode, ServerError> {
  server.sign_out(auth, None).await?;
  Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn revoke(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(id): Id<SessionId>,
) -> Result<StatusCode, ServerError> {
  server.sign_out(auth, Some(id)).await?;
  Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn password(
  State(server): State<Server>,
  Auth(auth): Auth,
  peer: ConnectInfo<PeerAddr>,
  headers: HeaderMap,
  SafeJson(input): SafeJson<PasswordChange>,
) -> Result<StatusCode, ServerError> {
  server
    .change_password(auth, input, address(&server, peer, &headers))
    .await?;
  Ok(StatusCode::NO_CONTENT)
}
