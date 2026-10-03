use axum::{
  Json,
  extract::{FromRequest, FromRequestParts, Path, Request, rejection::JsonRejection},
  http::{HeaderMap, request::Parts},
};
use orchester_api::{Credential, ErrorCode};
use serde::de::DeserializeOwned;

use crate::{Server, db::Authenticated, error::ServerError};

pub(super) struct SafeJson<T>(pub T);

impl<T: DeserializeOwned + Send> FromRequest<Server> for SafeJson<T> {
  type Rejection = ServerError;
  async fn from_request(request: Request, state: &Server) -> Result<Self, Self::Rejection> {
    // axum/serde rejection text can contain submitted values; the contract returns only a code.
    Json::<T>::from_request(request, state)
      .await
      .map(|Json(value)| Self(value))
      .map_err(|_: JsonRejection| ErrorCode::BadRequest.into())
  }
}

pub(super) struct Id<T>(pub T);

impl<T: DeserializeOwned + Send> FromRequestParts<Server> for Id<T> {
  type Rejection = ServerError;
  async fn from_request_parts(parts: &mut Parts, state: &Server) -> Result<Self, Self::Rejection> {
    Path::<T>::from_request_parts(parts, state)
      .await
      .map(|Path(id)| Self(id))
      .map_err(|_| ErrorCode::BadRequest.into())
  }
}

pub(super) struct Auth(pub Authenticated);

impl FromRequestParts<Server> for Auth {
  type Rejection = ServerError;
  async fn from_request_parts(parts: &mut Parts, _state: &Server) -> Result<Self, Self::Rejection> {
    parts
      .extensions
      .get::<Authenticated>()
      .cloned()
      .map(Self)
      .ok_or_else(|| ErrorCode::Unauthorized.into())
  }
}

pub(super) fn bearer(headers: &HeaderMap) -> Result<Credential, ServerError> {
  let mut values = headers.get_all("Authorization").iter();
  let header = values.next().ok_or(ErrorCode::Unauthorized)?;
  if values.next().is_some() {
    return Err(ErrorCode::Unauthorized.into());
  }
  let header = header.to_str().map_err(|_| ErrorCode::Unauthorized)?;
  let (scheme, token) = header.split_once(' ').ok_or(ErrorCode::Unauthorized)?;
  if !scheme.eq_ignore_ascii_case("Bearer")
    || token.len() != 68
    || !token.starts_with("orc_")
    || !token[4..].bytes().all(|byte| byte.is_ascii_hexdigit())
  {
    return Err(ErrorCode::Unauthorized.into());
  }
  Ok(Credential::new(token.to_owned()))
}
