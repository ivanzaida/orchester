use std::{future::Future, net::IpAddr, pin::Pin, time::Duration};

use actix_web::{
  FromRequest, HttpMessage, HttpRequest,
  dev::Payload,
  http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap},
  web,
};
use orchester_api::{Credential, cloud::CloudErrorCode};
use serde::de::DeserializeOwned;

use crate::{Cloud, accounts::Authenticated, addresses::client_address, error::CloudError};

type Extracted<T> = Pin<Box<dyn Future<Output = Result<T, CloudError>>>>;

/// Request bodies are limited to 1 MiB and must arrive within 10 seconds, as on the team server.
const BODY_LIMIT: usize = 1024 * 1024;

/// A JSON body. Rejections answer with a code only: serde's text can quote the submitted values.
pub(super) struct SafeJson<T>(pub T);

impl<T: DeserializeOwned + 'static> FromRequest for SafeJson<T> {
  type Error = CloudError;
  type Future = Extracted<Self>;

  fn from_request(request: &HttpRequest, payload: &mut Payload) -> Self::Future {
    let json = json_content(request.headers());
    let body = web::Payload::from_request(request, payload).into_inner();
    Box::pin(async move {
      if !json {
        return Err(CloudErrorCode::UnsupportedMediaType.into());
      }
      let body = body.map_err(|_| CloudError::from(CloudErrorCode::BadRequest))?;
      let bytes = tokio::time::timeout(Duration::from_secs(10), body.to_bytes_limited(BODY_LIMIT))
        .await
        .map_err(|_| CloudError::from(CloudErrorCode::RequestTimeout))?
        .map_err(|_| CloudError::from(CloudErrorCode::PayloadTooLarge))?
        .map_err(|_| CloudError::from(CloudErrorCode::BadRequest))?;
      serde_json::from_slice(&bytes)
        .map(Self)
        .map_err(|_| CloudErrorCode::BadRequest.into())
    })
  }
}

/// The signed-in device of the request's bearer token. Its account id goes into the request log.
pub(super) struct Device(pub Authenticated);

impl FromRequest for Device {
  type Error = CloudError;
  type Future = Extracted<Self>;

  fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
    let request = request.clone();
    Box::pin(async move {
      let token = bearer(request.headers())?;
      let auth = cloud(&request)?.authenticate(&token).await?;
      request.extensions_mut().insert(auth.account);
      Ok(Self(auth))
    })
  }
}

/// The client's address: the connection's, or the hop a trusted proxy added.
pub(super) struct ClientAddress(pub IpAddr);

impl FromRequest for ClientAddress {
  type Error = CloudError;
  type Future = std::future::Ready<Result<Self, CloudError>>;

  fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
    std::future::ready(address(request))
  }
}

fn address(request: &HttpRequest) -> Result<ClientAddress, CloudError> {
  let cloud = cloud(request)?;
  let peer = request.peer_addr().ok_or(CloudError::Worker)?.ip();
  Ok(ClientAddress(client_address(
    peer,
    &cloud.inner.config.trusted_proxies,
    request.headers(),
  )))
}

fn cloud(request: &HttpRequest) -> Result<Cloud, CloudError> {
  request
    .app_data::<web::Data<Cloud>>()
    .map(|data| Cloud::clone(data))
    .ok_or(CloudError::Worker)
}

/// `Content-Type: application/json`, with or without parameters (review 1, L2): a body sent as a form or as plain
/// text is refused, so a browser's simple cross-site request can never reach a JSON endpoint.
fn json_content(headers: &HeaderMap) -> bool {
  headers
    .get(CONTENT_TYPE)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.split(';').next())
    .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"))
}

fn bearer(headers: &HeaderMap) -> Result<Credential, CloudError> {
  let mut values = headers.get_all(AUTHORIZATION);
  let header = values.next().ok_or(CloudErrorCode::Unauthorized)?;
  if values.next().is_some() {
    return Err(CloudErrorCode::Unauthorized.into());
  }
  let header = header.to_str().map_err(|_| CloudErrorCode::Unauthorized)?;
  let (scheme, token) = header.split_once(' ').ok_or(CloudErrorCode::Unauthorized)?;
  if !scheme.eq_ignore_ascii_case("Bearer") {
    return Err(CloudErrorCode::Unauthorized.into());
  }
  Ok(Credential::new(token.to_owned()))
}
