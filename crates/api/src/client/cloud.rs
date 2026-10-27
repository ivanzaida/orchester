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

use super::{ClientError, CloudAddress, bearer, builder};
use crate::cloud::{
  AccountResponse, AccountServer, CloudError, CloudInvite, CloudInvites, CloudSignIn, CloudSignInChallenge,
  CloudSignedIn, IdentityTokenIssued, IdentityTokenRequest, KeySetResponse, RegisteredServer, ServerResponse,
  ServersResponse, TokenPurpose, TwoFactorSignIn, TwoFactorSignedIn,
};

/// What the cloud answered the password step of a sign-in (`POST /auth/sessions`).
#[derive(Debug)]
pub enum SignInAnswer {
  /// Signed in: the account has two-factor authentication off.
  SignedIn(CloudSignedIn),
  /// The password was right and the account has two-factor authentication on: a code finishes the sign-in
  /// ([`CloudClient::finish_sign_in`]) with this challenge.
  Challenged(CloudSignInChallenge),
}

/// Orchester Cloud's API, at the cloud's API address.
#[derive(Clone, Debug)]
pub struct CloudClient {
  http: reqwest::Client,
  address: CloudAddress,
}

impl CloudClient {
  /// A client of the cloud at `address`, through the operating system's trust store; plain HTTP only to this computer
  /// (a test cloud).
  pub fn new(address: CloudAddress) -> Result<Self, ClientError> {
    if address.api_host().needs_opt_in() {
      return Err(ClientError::PlainHttpRefused);
    }
    // reqwest's rustls takes the process's crypto provider; ring is the one the workspace builds.
    // Installing fails only when a provider is installed already, which then serves as well.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let http = builder().build().map_err(|error| ClientError::transport(&error))?;
    Ok(Self { http, address })
  }

  pub fn address(&self) -> &CloudAddress {
    &self.address
  }

  /// `POST /auth/sessions`: signs this computer in with the person's email and password; the device token in the
  /// answer is shown once. An account with two-factor authentication answers a challenge (`202`) instead.
  pub async fn sign_in(&self, request: &CloudSignIn) -> Result<SignInAnswer, ClientError> {
    let response = answered(self.json(Method::POST, "/auth/sessions", request)?).await?;
    if response.status() == StatusCode::ACCEPTED {
      return read(response).await.map(SignInAnswer::Challenged);
    }
    read(response).await.map(SignInAnswer::SignedIn)
  }

  /// `POST /auth/sessions/two-factor`: finishes a sign-in whose password step answered a challenge, with a code from
  /// the authenticator app or a recovery code. The device is a two-factor session.
  pub async fn finish_sign_in(&self, request: &TwoFactorSignIn) -> Result<TwoFactorSignedIn, ClientError> {
    call(self.json(Method::POST, "/auth/sessions/two-factor", request)?).await
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

  /// `POST /invites/{id}/decline`: the invite to the account's email ends at the cloud. The team server keeps its copy
  /// until it expires or is revoked; nothing joins with it without the invitee's own identity token.
  pub async fn decline_invite(&self, device: &SecretValue, invite: InviteId) -> Result<(), ClientError> {
    let path = format!("/invites/{invite}/decline");
    let request = self.http.request(Method::POST, self.address.endpoint(&path)?);
    send(bearer(request, device)).await
  }

  /// `GET /servers`: the team servers the account owns or joined.
  pub async fn servers(&self, device: &SecretValue) -> Result<Vec<AccountServer>, ClientError> {
    let request = self.http.request(Method::GET, self.address.endpoint("/servers")?);
    call::<ServersResponse>(bearer(request, device))
      .await
      .map(|answer| answer.servers)
  }

  /// `GET /servers/{id}`: a team server linked to the cloud, as registered: its address and certificate fingerprint.
  pub async fn server(&self, device: &SecretValue, server: TeamServerId) -> Result<RegisteredServer, ClientError> {
    let path = format!("/servers/{server}");
    let request = self.http.request(Method::GET, self.address.endpoint(&path)?);
    call::<ServerResponse>(bearer(request, device))
      .await
      .map(|answer| answer.server)
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

  /// `POST /identity-tokens`: a token to join the team server `audience`, valid five minutes, for the signed-in cloud
  /// device `device`; with `invite`, a token that accepts that open invite to the account's email.
  pub async fn identity_token(
    &self,
    device: &SecretValue,
    audience: TeamServerId,
    invite: Option<InviteId>,
  ) -> Result<IdentityTokenIssued, ClientError> {
    let request = IdentityTokenRequest {
      audience: audience.to_string(),
      purpose: None,
      invite,
    };
    self.issue(device, &request).await
  }

  /// `POST /identity-tokens` with purpose `code`: a token that shows the team server `audience`, with a code entered
  /// in the app, that this device signed in with two-factor authentication. Ask for it only after the server proved it
  /// holds the key whose id `audience` is ([`super::Client::key_proof`]).
  pub async fn code_token(
    &self,
    device: &SecretValue,
    audience: TeamServerId,
  ) -> Result<IdentityTokenIssued, ClientError> {
    let request = IdentityTokenRequest {
      audience: audience.to_string(),
      purpose: Some(TokenPurpose::Code),
      invite: None,
    };
    self.issue(device, &request).await
  }

  async fn issue(
    &self,
    device: &SecretValue,
    request: &IdentityTokenRequest,
  ) -> Result<IdentityTokenIssued, ClientError> {
    let body = serde_json::to_vec(request).map_err(|_| ClientError::Setup)?;
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
  read(answered(request).await?).await
}

/// The cloud's JSON answer of a success.
async fn read<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, ClientError> {
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
      ended: error.reason.map(|reason| (reason, error.ended_at)),
    },
    Err(_) => ClientError::NotOrchester { status },
  }
}
