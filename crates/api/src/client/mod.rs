//! The HTTP client the desktop talks to a server with (`docs/architecture/server.md`, "API",
//! "Security"): rustls with the operating system's trust store and no way to skip it, plain HTTP
//! only for this computer or with the person's opt-in, and redirects never followed but returned
//! as an error naming their target. No proxy is used, whatever the environment says. The token
//! travels only in the `Authorization` header, marked sensitive; it never appears in a URL, an
//! error or `Debug` output.

mod address;
mod cloud;
mod error;
mod pinned;
mod recorded;
mod sharing;

use std::time::Duration;

use orchester_core::{
  id::{SessionId, UserId},
  secret::SecretValue,
};
use reqwest::{
  Method, RequestBuilder, Response, StatusCode,
  header::{AUTHORIZATION, HeaderValue, LOCATION, RETRY_AFTER},
  redirect::Policy,
};
use serde::{Serialize, de::DeserializeOwned};
use zeroize::Zeroize;

#[cfg(feature = "test-support")]
pub use self::recorded::trust_for_tests;
pub use self::{
  address::{AddressError, PlainHttp, ServerAddress},
  cloud::{CloudClient, SignInAnswer},
  error::ClientError,
};
use crate::{
  ApiError, ApprovalResponse, CloudJoin, CloudJoinAnswer, CodeJoin, Joined, KeyProofRequest, KeyProofResponse,
  PairingCode, ServerInfo, Session, SessionsResponse, SharingErrorDetails, User, UserChanges, UserResponse,
  UsersResponse, certificate::CertificatePin,
};

/// How long making the connection may take (10.03 E: "the connection timed out after 10 s").
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a whole request may take; sign-in hashes a password on the server.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// One server's API.
#[derive(Clone)]
pub struct Client {
  http: reqwest::Client,
  address: ServerAddress,
  certificate: Certificate,
}

/// What a client knows of the certificate its connections present.
#[derive(Clone)]
enum Certificate {
  /// Exactly this one, pinned.
  Pinned(CertificatePin),
  /// The leaf of the last connection the trust store accepted; `None` before one, or over plain HTTP.
  Recorded(recorded::Presented),
}

impl std::fmt::Debug for Client {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Client").field("address", &self.address).finish()
  }
}

impl Client {
  /// A client of the server at `address`. Plain HTTP to another computer is refused unless the
  /// person allowed it for this server (`plain_http`).
  pub fn new(address: ServerAddress, plain_http: PlainHttp) -> Result<Self, ClientError> {
    if address.needs_opt_in() && plain_http == PlainHttp::NotAllowed {
      return Err(ClientError::PlainHttpRefused);
    }
    // reqwest's rustls takes the process's crypto provider; ring is the one the workspace builds.
    // Installing fails only when a provider is installed already, which then serves as well.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let presented = recorded::Presented::default();
    let tls = recorded::Recorded::config(presented.clone()).map_err(|_| ClientError::Setup)?;
    let http = builder()
      .tls_backend_preconfigured(tls)
      .build()
      .map_err(|error| ClientError::transport(&error))?;
    Ok(Self {
      http,
      address,
      certificate: Certificate::Recorded(presented),
    })
  }

  /// A client of the team server at the `https` `address` that accepts exactly the certificate `pin` names: the
  /// server's own certificate, pinned from a cloud invite or after the person confirmed its check code.
  pub fn pinned(address: ServerAddress, pin: CertificatePin) -> Result<Self, ClientError> {
    if !address.is_https() {
      return Err(ClientError::Setup);
    }
    let tls = pinned::PinnedVerifier::config(pin).map_err(|_| ClientError::Setup)?;
    let http = builder()
      .tls_backend_preconfigured(tls)
      .build()
      .map_err(|error| ClientError::transport(&error))?;
    Ok(Self {
      http,
      address,
      certificate: Certificate::Pinned(pin),
    })
  }

  /// The certificate the team server at the `https` `address` presents, read without trusting it, for the person to
  /// compare its check code before anything is sent there: the handshake ends at the certificate, so no request,
  /// token or code reaches the server.
  pub async fn presented_certificate(address: &ServerAddress) -> Result<CertificatePin, ClientError> {
    if !address.is_https() {
      return Err(ClientError::Setup);
    }
    let presented = std::sync::Arc::new(std::sync::OnceLock::new());
    let tls = pinned::Recorder::config(presented.clone()).map_err(|_| ClientError::Setup)?;
    let http = builder()
      .tls_backend_preconfigured(tls)
      .build()
      .map_err(|error| ClientError::transport(&error))?;
    let answer = http.get(address.endpoint("/server")?).send().await;
    match (presented.get(), answer) {
      (Some(pin), _) => Ok(*pin),
      (None, Err(error)) => Err(ClientError::transport(&error)),
      // A handshake always shows the certificate first; an answer without one came over another protocol.
      (None, Ok(response)) => Err(ClientError::NotOrchester {
        status: response.status().as_u16(),
      }),
    }
  }

  pub fn address(&self) -> &ServerAddress {
    &self.address
  }

  /// The certificate this client's connections present: the pinned one, or the leaf of the last connection the
  /// operating system's store accepted (its chain and name checked); `None` before any, or over plain HTTP. Read after
  /// a request, it is that request's connection's.
  pub fn certificate(&self) -> Option<CertificatePin> {
    match &self.certificate {
      Certificate::Pinned(pin) => Some(*pin),
      Certificate::Recorded(presented) => *presented.lock(),
    }
  }

  /// `GET /server`: what answers at the address, asked before signing in.
  pub async fn server_info(&self) -> Result<ServerInfo, ClientError> {
    self.call(self.request(Method::GET, "/server")?).await
  }

  /// `POST /server/key-proof`: the server's public key and its signature of `nonce`, which the caller verifies. Names
  /// the certificate this client's connections last presented, when it knows one ([`crate::PRESENTED_CERTIFICATE`]).
  pub async fn key_proof(&self, nonce: &str) -> Result<KeyProofResponse, ClientError> {
    let request = KeyProofRequest {
      nonce: nonce.to_owned(),
    };
    let mut request = self.json(Method::POST, "/server/key-proof", &request)?;
    if let Some(presented) = self.certificate() {
      request = request.header(crate::PRESENTED_CERTIFICATE, presented.to_hex());
    }
    self.call(request).await
  }

  /// `POST /auth/code`: joins with a setup, invite, account or pairing code and returns this device's token.
  pub async fn join_with_code(&self, request: &CodeJoin) -> Result<Joined, ClientError> {
    self.call(self.json(Method::POST, "/auth/code", request)?).await
  }

  /// `POST /auth/cloud`: joins with an Orchester Cloud identity token. `Waiting` while join approval holds the join;
  /// asking again with the approval's id finishes it once approved.
  pub async fn join_with_cloud(&self, request: &CloudJoin) -> Result<CloudJoinAnswer, ClientError> {
    let response = answered(self.json(Method::POST, "/auth/cloud", request)?).await?;
    let status = response.status().as_u16();
    let bytes = response.bytes().await.map_err(|error| ClientError::transport(&error))?;
    let answer = match status {
      201 => serde_json::from_slice::<Joined>(&bytes).map(CloudJoinAnswer::Joined),
      202 => {
        serde_json::from_slice::<ApprovalResponse>(&bytes).map(|waiting| CloudJoinAnswer::Waiting(waiting.approval))
      }
      _ => return Err(ClientError::NotOrchester { status }),
    };
    answer.map_err(|_| ClientError::NotOrchester { status })
  }

  /// `POST /auth/pairings`: a code for this person's next computer, valid 10 minutes.
  pub async fn pairing_code(&self, token: &SecretValue) -> Result<PairingCode, ClientError> {
    self
      .call(bearer(self.request(Method::POST, "/auth/pairings")?, token))
      .await
  }

  /// `GET /auth/me`: the signed-in user; `401` once the token expired or was revoked.
  pub async fn me(&self, token: &SecretValue) -> Result<User, ClientError> {
    let response: UserResponse = self.call(bearer(self.request(Method::GET, "/auth/me")?, token)).await?;
    Ok(response.user)
  }

  pub async fn sessions(&self, token: &SecretValue) -> Result<Vec<Session>, ClientError> {
    let response: SessionsResponse = self
      .call(bearer(self.request(Method::GET, "/auth/sessions")?, token))
      .await?;
    Ok(response.sessions)
  }

  /// `DELETE /auth/sessions/current`: the token stops working.
  pub async fn sign_out(&self, token: &SecretValue) -> Result<(), ClientError> {
    self
      .send(bearer(self.request(Method::DELETE, "/auth/sessions/current")?, token))
      .await
  }

  /// `DELETE /auth/sessions/{id}`: signs one of the person's other computers out.
  pub async fn revoke_session(&self, token: &SecretValue, session: SessionId) -> Result<(), ClientError> {
    let path = format!("/auth/sessions/{session}");
    self.send(bearer(self.request(Method::DELETE, &path)?, token)).await
  }

  pub async fn users(&self, token: &SecretValue) -> Result<Vec<User>, ClientError> {
    let response: UsersResponse = self
      .call(bearer(self.request(Method::GET, "/admin/users")?, token))
      .await?;
    Ok(response.users)
  }

  pub async fn update_user(
    &self,
    token: &SecretValue,
    user: UserId,
    changes: &UserChanges,
  ) -> Result<User, ClientError> {
    let path = format!("/admin/users/{user}");
    let response: UserResponse = self
      .call(bearer(self.json(Method::PATCH, &path, changes)?, token))
      .await?;
    Ok(response.user)
  }

  fn request(&self, method: Method, path: &str) -> Result<RequestBuilder, ClientError> {
    Ok(self.http.request(method, self.address.endpoint(path)?))
  }

  fn json<T: Serialize>(&self, method: Method, path: &str, body: &T) -> Result<RequestBuilder, ClientError> {
    // Serializing the contract's own types fails only for a map with non-string keys, which it has not.
    let bytes = serde_json::to_vec(body).map_err(|_| ClientError::Setup)?;
    Ok(
      self
        .request(method, path)?
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(bytes),
    )
  }

  /// Sends `request` and reads the JSON answer of a successful call.
  async fn call<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, ClientError> {
    let response = answered(request).await?;
    let status = response.status().as_u16();
    let bytes = response.bytes().await.map_err(|error| ClientError::transport(&error))?;
    // An answer that is not the contract is not an Orchester server; nothing of it is kept.
    serde_json::from_slice(&bytes).map_err(|_| ClientError::NotOrchester { status })
  }

  /// Sends `request` whose success has no body (`204`).
  async fn send(&self, request: RequestBuilder) -> Result<(), ClientError> {
    answered(request).await.map(drop)
  }
}

/// No proxy, from the environment (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`) or the system: a plain HTTP proxy would
/// see the token, and a server on this computer would be reached through another one.
fn builder() -> reqwest::ClientBuilder {
  reqwest::Client::builder()
    .no_proxy()
    .redirect(Policy::none())
    .connect_timeout(CONNECT_TIMEOUT)
    .timeout(REQUEST_TIMEOUT)
}

/// `request` with the bearer token, as a header value marked sensitive so it is never shown.
fn bearer(request: RequestBuilder, token: &SecretValue) -> RequestBuilder {
  let mut text = format!("Bearer {}", token.expose());
  let value = HeaderValue::from_str(&text);
  text.zeroize();
  match value {
    Ok(mut value) => {
      value.set_sensitive(true);
      request.header(AUTHORIZATION, value)
    }
    // A token the server issued is always visible ASCII; another value is not sent at all, and
    // the server answers 401 as for a token it does not know.
    Err(_) => request,
  }
}

/// Sends `request`: the response of a success, or the error of a redirect, a refusal or a failed
/// transport.
async fn answered(request: RequestBuilder) -> Result<Response, ClientError> {
  let response = request.send().await.map_err(|error| ClientError::transport(&error))?;
  let status = response.status();
  if status.is_success() {
    return Ok(response);
  }
  if status.is_redirection() {
    let location = response
      .headers()
      .get(LOCATION)
      .and_then(|value| value.to_str().ok())
      .map(str::to_owned);
    return Err(ClientError::Redirect { location });
  }
  Err(refusal(response, status).await)
}

/// The error of an answer that is neither a success nor a redirect.
async fn refusal(response: Response, status: StatusCode) -> ClientError {
  let retry_after = response
    .headers()
    .get(RETRY_AFTER)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.trim().parse().ok());
  let status = status.as_u16();
  let Ok(bytes) = response.bytes().await else {
    return ClientError::NotOrchester { status };
  };
  match serde_json::from_slice::<ApiError>(&bytes) {
    Ok(error) => ClientError::Api {
      status,
      code: error.error,
      retry_after,
      details: details(&bytes).map(Box::new),
    },
    Err(_) => ClientError::NotOrchester { status },
  }
}

/// The details of a refusal: its fields beside the code and the English message. A refusal without
/// any, or with fields the contract does not know, has none.
fn details(bytes: &[u8]) -> Option<SharingErrorDetails> {
  let serde_json::Value::Object(mut fields) = serde_json::from_slice(bytes).ok()? else {
    return None;
  };
  fields.remove("error");
  fields.remove("message");
  if fields.is_empty() {
    return None;
  }
  serde_json::from_value(serde_json::Value::Object(fields)).ok()
}
