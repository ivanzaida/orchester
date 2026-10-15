//! Orchester Cloud's JSON contract (`docs/architecture/server.md`, "Orchester Cloud"; `docs/cloud-api.md`): accounts,
//! devices and identity tokens. Used by the cloud, the desktop and the web front alike.

use orchester_core::id::{CloudAccountId, CloudDeviceId, TeamServerId};
use serde::{Deserialize, Serialize};

use crate::Credential;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignUp {
  pub email: String,
  pub password: Credential,
  pub display_name: String,
}

/// The token of a confirmation link, as the web front read it from the link (`/confirm/{token}`), and the password
/// the person chose when signing up: a link is confirmed only with its own sign-up's password.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmEmail {
  pub link_token: Credential,
  pub password: Credential,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudPlatform {
  Windows,
  Macos,
  Linux,
  /// A browser signed in through the web front.
  Web,
}

impl CloudPlatform {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Windows => "windows",
      Self::Macos => "macos",
      Self::Linux => "linux",
      Self::Web => "web",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    [Self::Windows, Self::Macos, Self::Linux, Self::Web]
      .into_iter()
      .find(|platform| platform.as_str() == value)
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceInfo {
  pub name: String,
  pub platform: CloudPlatform,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudSignIn {
  pub email: String,
  pub password: Credential,
  pub device: DeviceInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudAccount {
  pub id: CloudAccountId,
  pub email: String,
  pub display_name: String,
  pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudDevice {
  pub id: CloudDeviceId,
  pub name: String,
  pub platform: CloudPlatform,
  pub created_at: i64,
  pub last_used_at: i64,
  pub current: bool,
}

/// The answer to a sign-in. The token is shown once; the cloud keeps only its hash.
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudSignedIn {
  pub token: Credential,
  pub device: CloudDevice,
  pub account: CloudAccount,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetRequest {
  pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetFinish {
  pub link_token: Credential,
  pub new_password: Credential,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountResponse {
  pub account: CloudAccount,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountChanges {
  pub display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DevicesResponse {
  pub devices: Vec<CloudDevice>,
}

/// `audience` is the id of the team server the token is for; the team server accepts only its own.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityTokenRequest {
  pub audience: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IdentityTokenIssued {
  pub identity_token: Credential,
  /// Seconds since the Unix epoch, as the token's own `exp`.
  pub expires_at: i64,
}

/// A team server as Orchester Cloud names it (`GET /servers`, an accepted invite): its id, which identity tokens for
/// it carry as their audience, its address, and the SHA-256 of its own certificate in hex, or `None` for a server with
/// a CA's certificate. An app connects to that address trusting exactly that certificate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudServer {
  pub id: TeamServerId,
  pub name: String,
  pub address: String,
  pub fingerprint: Option<String>,
}

/// The current key set as a compact JWS signed by the cloud's root key (`identity::KeySet` verifies it).
#[derive(Debug, Serialize, Deserialize)]
pub struct KeySetResponse {
  pub key_set: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudErrorCode {
  BadRequest,
  PayloadTooLarge,
  MethodNotAllowed,
  RequestTimeout,
  /// A request body that is not `application/json`.
  UnsupportedMediaType,
  NotFound,
  Unauthorized,
  InvalidCredentials,
  EmailUnconfirmed,
  /// A confirmation or reset link that is unknown, used or expired: one answer for all three.
  InvalidLink,
  Invalid,
  RateLimited,
  /// No published key set lists a signing key the cloud may use now.
  KeysUnavailable,
  /// The cloud is at a capacity limit (its limiters' memory, its password hashing); try again shortly.
  Busy,
  Internal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloudError {
  pub error: CloudErrorCode,
  /// English diagnostics; the desktop and the web front show their own text by code.
  pub message: String,
}
