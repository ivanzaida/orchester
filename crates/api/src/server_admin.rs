//! The team server's own settings for its admins (`docs/architecture/server.md`, "The server admin", "Identity tokens
//! and keys"): the cloud link and join approval.

use serde::{Deserialize, Serialize};

use crate::{CertificateInfo, Credential, UserRef};

/// The server's link to Orchester Cloud.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudLink {
  /// The server's own id: the audience of identity tokens for it.
  pub server: String,
  /// The base64url SHA-256 of the pinned root key, `null` while nothing is pinned.
  pub root_fingerprint: Option<String>,
  pub accepting: bool,
  /// When the cached key set expires; cloud joins stop then until a newer one arrives.
  pub key_set_expires_at: Option<i64>,
  /// When the cached key set was issued: the last time the server got a newer one.
  #[serde(default)]
  pub key_set_issued_at: Option<i64>,
  pub linked_at: Option<i64>,
  /// The admin who linked it.
  #[serde(default)]
  pub linked_by: Option<UserRef>,
}

/// What finishing a link brings back from the cloud: its root key (pinned now) and a key set the root signed. With an
/// identity token, the calling admin's account is linked to that cloud identity too.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudLinkFinish {
  pub root_key: String,
  pub key_set: String,
  pub identity_token: Option<Credential>,
}

/// `link` of `PATCH /server/cloud`: `"start"` returns the server's signed link statement for the cloud; the finish
/// pins the cloud's root.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CloudLinkStep {
  Start(LinkStart),
  Finish(CloudLinkFinish),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkStart {
  Start,
}

/// `PATCH /server/cloud`. `pin` replaces the pinned root key (after a root compromise) with this base64url public key.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudLinkChanges {
  pub accepting: Option<bool>,
  pub link: Option<CloudLinkStep>,
  #[serde(default)]
  pub unlink: bool,
  pub pin: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloudLinkResponse {
  pub cloud: CloudLink,
  /// The signed link statement, after `link: "start"`.
  pub statement: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerSettings {
  /// A cloud join of an existing account waits for that account's devices or an admin (row 57).
  pub join_approval: bool,
  /// The server's name, shown to everyone it invites; `name` in `server.toml` until an admin changes it.
  #[serde(default)]
  pub name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsChanges {
  pub join_approval: Option<bool>,
  pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
  pub settings: ServerSettings,
  /// The certificate's check code, `null` when the server serves plain HTTP behind a proxy.
  pub check_code: Option<String>,
  #[serde(default)]
  pub certificate: Option<CertificateInfo>,
}
