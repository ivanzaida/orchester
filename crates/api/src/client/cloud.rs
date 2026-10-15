//! What the desktop asks Orchester Cloud for (`docs/architecture/server.md`, "Orchester Cloud", "Identity tokens and
//! keys", "Invites"): signing this computer in and out, the account, its invites and team servers, an identity token
//! for one team server, and the cloud's current key set, which the app carries to a team server that cannot reach the
//! cloud itself. The cloud device token travels only in the `Authorization` header.

use orchester_core::{
  id::{InviteId, TeamServerId},
  secret::SecretValue,
};
use reqwest::{
  Method, RequestBuilder, StatusCode,
  header::{CONTENT_TYPE, LOCATION, RETRY_AFTER},
};
use serde::de::DeserializeOwned;

use super::{ClientError, ServerAddress, bearer, builder};
use crate::cloud::{
  AccountResponse, CloudError, CloudInvite, CloudInvites, CloudServer, CloudServers, CloudSignIn, CloudSignedIn,
  IdentityTokenIssued, IdentityTokenRequest, InviteAccepted, KeySetResponse,
};

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

  pub fn address(&self) -> &ServerAddress {
    &self.address
  }

  /// `POST /auth/sessions`: signs this computer in with the person's email and password; the device token in the
  /// answer is shown once.
  pub async fn sign_in(&self, request: &CloudSignIn) -> Result<CloudSignedIn, ClientError> {
    call(self.json(Method::POST, "/auth/sessions", request)?).await
  }

  /// `DELETE /auth/sessions/current`: ends the device token `device`.
  pub async fn sign_out(&self, device: &SecretValue) -> Result<(), ClientError> {
    let request = self
      .http
      .request(Method::DELETE, self.address.endpoint("/auth/sessions/current")?);
    send(bearer(request, device)).await
  }

  /// `GET /me`: the account the device token `device` is signed in to.
  pub async fn me(&self, device: &SecretValue) -> Result<AccountResponse, ClientError> {
    let request = self.http.request(Method::GET, self.address.endpoint("/me")?);
    call(bearer(request, device)).await
  }

  /// `GET /invites`: the invites waiting for the account.
  pub async fn invites(&self, device: &SecretValue) -> Result<Vec<CloudInvite>, ClientError> {
    let request = self.http.request(Method::GET, self.address.endpoint("/invites")?);
    call::<CloudInvites>(bearer(request, device))
      .await
      .map(|answer| answer.invites)
  }

  /// `POST /invites/{id}/accept`: the invite's server and a one-time identity token for it.
  pub async fn accept_invite(&self, device: &SecretValue, invite: InviteId) -> Result<InviteAccepted, ClientError> {
    let path = format!("/invites/{invite}/accept");
    let request = self.http.request(Method::POST, self.address.endpoint(&path)?);
    call(bearer(request, device)).await
  }

  /// `POST /invites/{id}/decline`: the invite ends; the team server learns it was declined.
  pub async fn decline_invite(&self, device: &SecretValue, invite: InviteId) -> Result<(), ClientError> {
    let path = format!("/invites/{invite}/decline");
    let request = self.http.request(Method::POST, self.address.endpoint(&path)?);
    send(bearer(request, device)).await
  }

  /// `GET /servers`: the team servers the account owns or joined.
  pub async fn servers(&self, device: &SecretValue) -> Result<Vec<CloudServer>, ClientError> {
    let request = self.http.request(Method::GET, self.address.endpoint("/servers")?);
    call::<CloudServers>(bearer(request, device))
      .await
      .map(|answer| answer.servers)
  }

  fn json<T: serde::Serialize>(&self, method: Method, path: &str, body: &T) -> Result<RequestBuilder, ClientError> {
    // Serializing the contract's own types fails only for a map with non-string keys, which it has not.
    let bytes = serde_json::to_vec(body).map_err(|_| ClientError::Setup)?;
    Ok(
      self
        .http
        .request(method, self.address.endpoint(path)?)
        .header(CONTENT_TYPE, "application/json")
        .body(bytes),
    )
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

/// Sends `request` whose success has no body (`204`), and reads the cloud's refusal.
async fn send(request: RequestBuilder) -> Result<(), ClientError> {
  answered(request).await.map(drop)
}

/// Sends `request` and reads the cloud's JSON answer, or its refusal.
async fn call<T: DeserializeOwned>(request: RequestBuilder) -> Result<T, ClientError> {
  let response = answered(request).await?;
  let code = response.status().as_u16();
  let bytes = response.bytes().await.map_err(|error| ClientError::transport(&error))?;
  // An answer that is not the contract is not Orchester Cloud; nothing of it is kept.
  serde_json::from_slice(&bytes).map_err(|_| ClientError::NotOrchester { status: code })
}

/// Sends `request`: the response of a success, or the error of a redirect, a refusal or a failed transport.
async fn answered(request: RequestBuilder) -> Result<reqwest::Response, ClientError> {
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
  if status.is_success() {
    return Ok(response);
  }
  let bytes = response.bytes().await.map_err(|error| ClientError::transport(&error))?;
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
