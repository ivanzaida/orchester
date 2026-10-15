use axum::{
  Json,
  extract::{ConnectInfo, State},
  http::{HeaderMap, StatusCode},
  response::{IntoResponse, Response},
};
use orchester_api::{
  ApprovalDecision, ApprovalResponse, Approvals, CloudJoin, CodeJoin, PairingCode, SessionsResponse, UserResponse,
};
use orchester_core::id::{ApprovalId, SessionId};

use super::{
  address,
  extract::{Auth, Id, SafeJson, bearer},
};
use crate::{Server, accounts::CloudJoined, error::ServerError, tls::PeerAddr};

pub(super) async fn join_with_code(
  State(server): State<Server>,
  peer: ConnectInfo<PeerAddr>,
  headers: HeaderMap,
  SafeJson(input): SafeJson<CodeJoin>,
) -> Result<Response, ServerError> {
  let joined = server.join_with_code(input, address(&server, peer, &headers)).await?;
  let id = joined.user.id;
  let mut response = (StatusCode::CREATED, Json(joined)).into_response();
  response.extensions_mut().insert(id);
  Ok(response)
}

/// `POST /auth/cloud`: no token to join; the caller's device token to link their account to the identity.
pub(super) async fn join_with_cloud(
  State(server): State<Server>,
  peer: ConnectInfo<PeerAddr>,
  headers: HeaderMap,
  SafeJson(input): SafeJson<CloudJoin>,
) -> Result<Response, ServerError> {
  let caller = if headers.contains_key("Authorization") {
    Some(server.authenticate(bearer(&headers)?).await?)
  } else {
    None
  };
  let address = address(&server, peer, &headers);
  let (id, response) = match server.join_with_cloud(input, caller, address).await? {
    CloudJoined::Joined(joined) => (joined.user.id, (StatusCode::CREATED, Json(joined)).into_response()),
    CloudJoined::Waiting(approval) => (
      approval.user.id,
      (StatusCode::ACCEPTED, Json(ApprovalResponse { approval })).into_response(),
    ),
    CloudJoined::Linked(user) => (user.id, Json(UserResponse { user }).into_response()),
  };
  let mut response = response;
  response.extensions_mut().insert(id);
  Ok(response)
}

pub(super) async fn pairing(
  State(server): State<Server>,
  Auth(auth): Auth,
) -> Result<(StatusCode, Json<PairingCode>), ServerError> {
  Ok((StatusCode::CREATED, Json(server.pairing_code(auth).await?)))
}

pub(super) async fn approvals(State(server): State<Server>, Auth(auth): Auth) -> Result<Json<Approvals>, ServerError> {
  Ok(Json(Approvals {
    approvals: server.approvals(auth).await?,
  }))
}

pub(super) async fn decide(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(id): Id<ApprovalId>,
  SafeJson(input): SafeJson<ApprovalDecision>,
) -> Result<Json<ApprovalResponse>, ServerError> {
  Ok(Json(ApprovalResponse {
    approval: server.decide_approval(auth, id, input.approve).await?,
  }))
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
