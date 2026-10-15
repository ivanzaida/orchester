//! What the desktop asks Orchester Cloud for when it joins a team server (`docs/architecture/server.md`, "Identity
//! tokens and keys", "Orchester Cloud"): an identity token for that one server, and the cloud's current key set, which
//! the app carries to a team server that cannot reach the cloud itself. The cloud device token travels only in the
//! `Authorization` header.

use orchester_core::{id::TeamServerId, secret::SecretValue};
use reqwest::{
  Method, RequestBuilder, StatusCode,
  header::{CONTENT_TYPE, LOCATION, RETRY_AFTER},
};
use serde::de::DeserializeOwned;

use super::{ClientError, ServerAddress, bearer, builder};
use crate::cloud::{CloudError, IdentityTokenIssued, IdentityTokenRequest, KeySetResponse};

/// Orchester Cloud's API at one address.
#[derive(Clone, Debug)]
pub struct CloudClient {
  http: reqwest::Client,
  address: ServerAddress,
}

impl CloudClient {
  /// A client of the cloud at `address`, through the operating system's trust store; plain HTTP only to this computer
  /// (a test cloud).
  pub fn new(address: ServerAddress) -> Result<Self, ClientError> {
    if address.needs_opt_in() {
      return Err(ClientError::PlainHttpRefused);
    }
    // reqwest's rustls takes the process's crypto provider; ring is the one the workspace builds.
    // Installing fails only when a provider is installed already, which then serves as well.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let http = builder().build().map_err(|error| ClientError::transport(&error))?;
    Ok(Self { http, address })
  }

  /// `POST /identity-tokens`: a token for the team server `audience`, valid five minutes, for the signed-in cloud
  /// device `device`.
  pub async fn identity_token(
    &self,
    device: &SecretValue,
    audience: TeamServerId,
  ) -> Result<IdentityTokenIssued, ClientError> {
    let body = serde_json::to_vec(&IdentityTokenRequest {
      audience: audience.to_string(),
      purpose: None,
    })
    .map_err(|_| ClientError::Setup)?;
    let request = self
      .http
      .request(Method::POST, self.address.endpoint("/identity-tokens")?)
      .header(CONTENT_TYPE, "application/json")
      .body(body);
    call(bearer(request, device)).await
  }

  /// `GET /keys`: the cloud's current key set, signed by its root key.
  pub async fn key_set(&self) -> Result<KeySetResponse, ClientError> {
    call(self.http.request(Method::GET, self.address.endpoint("/keys")?)).await
  }
}

/// Sends `request` and reads the cloud's JSON answer, or its refusal.
async fn call<T: DeserializeOwned>(request: RequestBuilder) -> Result<T, ClientError> {
  let response = request.send().await.map_err(|error| ClientError::transport(&error))?;
  let status = response.status();
  if status.is_redirection() {
    let location = response
      .headers()
      .get(LOCATION)
      .and_then(|value| value.to_str().ok())
      .map(str::to_owned);
    return Err(ClientError::Redirect { location });
  }
  let retry_after = response
    .headers()
    .get(RETRY_AFTER)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.trim().parse().ok());
  let bytes = response.bytes().await.map_err(|error| ClientError::transport(&error))?;
  let code = status.as_u16();
  if status.is_success() {
    // An answer that is not the contract is not Orchester Cloud; nothing of it is kept.
    return serde_json::from_slice(&bytes).map_err(|_| ClientError::NotOrchester { status: code });
  }
  Err(refusal(&bytes, status, retry_after))
}

fn refusal(bytes: &[u8], status: StatusCode, retry_after: Option<u64>) -> ClientError {
  let status = status.as_u16();
  match serde_json::from_slice::<CloudError>(bytes) {
    Ok(error) => ClientError::Cloud {
      status,
      code: error.error,
      retry_after,
    },
    Err(_) => ClientError::NotOrchester { status },
  }
}
