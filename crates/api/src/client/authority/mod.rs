//! A team server's workspace-authority surface under `/api/v2` (`docs/cloud-api.md`, "Team authority S10 read
//! contract", "Team authority S11 subscriptions"): the bounded reads ([`reads`]), the scoped invalidation
//! subscription ([`subscription`]), the team commands with their operations ([`commands`]) and the routes of authorized
//! work ([`work`]), through [`Client::authority`]. Refusals new to `/api/v2` are [`AuthorityError`]s and come as
//! [`AuthorityCallError::Refused`]; the common failures keep the `/api/v1` `{error, message}` form and come as
//! [`ClientError::Api`]. The routes are listed in `docs/cloud-api.md`, "Team authority S10/S11 routes", "Team
//! authority P2 commands" and "Team authority P4 work".

mod commands;
mod reads;
mod subscription;
mod work;

use orchester_core::{id::ProjectId, secret::SecretValue};
use reqwest::{Method, RequestBuilder, Response};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

pub use self::{
  commands::PolicyChange,
  subscription::{Subscription, SubscriptionEnd},
  work::BundleReadError,
};
use super::{Client, ClientError, bearer, redirect, refused, retry_after};
use crate::v2::{AuthorityError, AuthorityErrorCode};

/// Where the workspace-authority API is under a team server's address.
const V2_BASE: &str = "/api/v2";
/// The header every `/api/v2` request names the desktop's version in, a `ReleaseVersion` (`0.5.0`): a server refuses a
/// desktop older than it accepts `426 unsupported_contract`, with the version it needs (`docs/cloud-api.md`).
pub const CLIENT_VERSION_HEADER: &str = "Orchester-Client-Version";

/// What can go wrong calling the `/api/v2` surface. No variant carries a request or response body, a header or a
/// token.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AuthorityCallError {
  /// The request did not reach the server, or what answered is not an Orchester server.
  #[error(transparent)]
  Client(#[from] ClientError),
  /// The server refused the request with an [`AuthorityError`]; `retry_after` comes with `rate_limited` and
  /// `unavailable`, in seconds.
  #[error("the server refused the request: {:?} (HTTP {status})", error.code())]
  Refused {
    status: u16,
    error: AuthorityError,
    retry_after: Option<u64>,
  },
}

impl AuthorityCallError {
  /// The refusal's code, when the server refused the request.
  pub fn code(&self) -> Option<AuthorityErrorCode> {
    match self {
      Self::Refused { error, .. } => Some(error.code()),
      Self::Client(_) => None,
    }
  }
}

/// One team project's `/api/v2` surface, signed in with `token`.
pub struct Authority<'a> {
  client: &'a Client,
  token: &'a SecretValue,
  project: ProjectId,
}

impl Client {
  /// The workspace-authority surface of `project` on this server, signed in with `token`.
  pub fn authority<'a>(&'a self, token: &'a SecretValue, project: ProjectId) -> Authority<'a> {
    Authority {
      client: self,
      token,
      project,
    }
  }
}

impl Authority<'_> {
  /// A request to `route` under the project, e.g. `/task-list`, with the bearer token and the client's version.
  fn request(&self, method: Method, route: &str) -> Result<RequestBuilder, ClientError> {
    let path = format!("/projects/{}{route}", self.project);
    let url = self.client.address.endpoint_under(V2_BASE, &path)?;
    let request = bearer(self.client.http.request(method, url), self.token);
    Ok(match &self.client.version {
      Some(version) => request.header(CLIENT_VERSION_HEADER, version.as_str()),
      None => request,
    })
  }

  /// `GET` of `route`, read as `T`.
  async fn get<T: DeserializeOwned>(&self, route: &str) -> Result<T, AuthorityCallError> {
    call(self.request(Method::GET, route)?).await
  }

  /// `POST` of `body` to `route`, the answer read as `T`: the listings that take a request with filters and a page,
  /// and the commands.
  async fn query<B: Serialize, T: DeserializeOwned>(&self, route: &str, body: &B) -> Result<T, AuthorityCallError> {
    // Serializing the contract's own types fails only for a map with non-string keys, which it has not.
    let bytes = serde_json::to_vec(body).map_err(|_| ClientError::Setup)?;
    let request = self
      .request(Method::POST, route)?
      .header(reqwest::header::CONTENT_TYPE, "application/json")
      .body(bytes);
    call(request).await
  }
}

/// Sends `request` and reads the JSON answer of a successful call.
async fn call<T: DeserializeOwned>(request: RequestBuilder) -> Result<T, AuthorityCallError> {
  let response = answered(request).await?;
  let status = response.status().as_u16();
  let bytes = response.bytes().await.map_err(|error| ClientError::transport(&error))?;
  // An answer that is not the contract is not an Orchester server; nothing of it is kept.
  serde_json::from_slice(&bytes).map_err(|_| ClientError::NotOrchester { status }.into())
}

/// Sends `request`: the response of a success, or the error of a redirect, a refusal or a failed transport. The
/// server answers what is new to `/api/v2` with an [`AuthorityError`] and the common failures (`401`, `404`, a
/// malformed body's `400`) with their `/api/v1` `{error, message}` envelope, read as [`ClientError::Api`].
async fn answered(request: RequestBuilder) -> Result<Response, AuthorityCallError> {
  let response = request.send().await.map_err(|error| ClientError::transport(&error))?;
  let status = response.status();
  if status.is_success() {
    return Ok(response);
  }
  if status.is_redirection() {
    return Err(redirect(&response).into());
  }
  let retry_after = retry_after(&response);
  let status = status.as_u16();
  let Ok(bytes) = response.bytes().await else {
    return Err(ClientError::NotOrchester { status }.into());
  };
  match serde_json::from_slice::<AuthorityError>(&bytes) {
    Ok(error) => Err(AuthorityCallError::Refused {
      status,
      error,
      retry_after,
    }),
    Err(_) => Err(refused(&bytes, status, retry_after).into()),
  }
}
