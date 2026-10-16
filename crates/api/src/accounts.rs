use orchester_core::id::{SessionId, UserId};
use serde::{Deserialize, Serialize};

use crate::Credential;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerInfo {
  pub name: String,
  pub api_version: u8,
  /// `needs_setup` on the wire since API 3: the server has no admin yet and prints a setup code. The Rust name is
  /// API 2's while the frozen `crates/desktop` reads it.
  #[serde(rename = "needs_setup")]
  pub needs_bootstrap: bool,
  /// The cloud link, `null` when the server is not linked to Orchester Cloud.
  pub cloud: Option<CloudStatus>,
  /// The certificate the server presents, for a browser to compare before it trusts it; `null` when the server
  /// serves plain HTTP behind a proxy that holds the certificate.
  #[serde(default)]
  pub certificate: Option<CertificateInfo>,
  /// The server's own version, `0.x.y`.
  #[serde(default)]
  pub version: String,
}

/// How the server's certificate came to be (`tls` in `server.toml`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertificateMode {
  /// Made by the server itself on its first start.
  SelfSigned,
  /// The team's own certificate.
  Pem,
}

/// The certificate the server presents: its check code and the SHA-256 of its DER in lowercase hex, the value a
/// browser shows as the certificate's fingerprint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateInfo {
  pub check_code: String,
  pub sha256: String,
  pub mode: CertificateMode,
}

/// What `GET /server` says about the cloud link, before anyone signs in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudStatus {
  /// The server's id: the audience of identity tokens for it.
  #[serde(default)]
  pub server: String,
  /// The base64url SHA-256 of the cloud's root key the server pinned.
  pub root_fingerprint: String,
  /// Whether the server accepts identity tokens now (the owner can turn it off).
  pub accepting: bool,
  /// Every code but the setup code needs the app's identity token for codes with the two-factor claim
  /// (`POST /auth/code`). A server from before two-factor authentication lacks this: it takes a code alone and
  /// refuses a join that carries the token (its `CodeJoin` has no such fields), so the app sends none to it.
  #[serde(default)]
  pub codes_need_two_factor: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
  pub id: UserId,
  pub username: String,
  pub display_name: String,
  pub admin: bool,
  pub disabled: bool,
  /// Whether the account is linked to a cloud identity (API 3).
  pub cloud: bool,
}

/// How a device token was issued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinedVia {
  Cloud,
  Code,
  /// A session signed in with a password before API 3, kept by the migration.
  #[serde(rename = "before_3")]
  Before3,
}

/// What a device token is for. An `admin_web` token is a browser signed in to the server admin: it lives 12 hours from
/// sign-in, reaches only what the admin pages call, and is not listed among the account's computers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
  #[default]
  App,
  AdminWeb,
}

/// A device token of the caller's account.
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
  pub id: SessionId,
  pub device: String,
  pub created_at: i64,
  pub last_used_at: i64,
  pub current: bool,
  pub via: JoinedVia,
}

/// A password change: Orchester Cloud's `POST /auth/password` (team servers have no passwords since API 3).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasswordChange {
  pub current_password: Credential,
  pub new_password: Credential,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
  pub user: User,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionsResponse {
  pub sessions: Vec<Session>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsersResponse {
  pub users: Vec<User>,
}

/// An account as the server admin's People lists it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminUser {
  #[serde(flatten)]
  pub user: User,
  /// How many projects the account is a member of.
  pub projects: u32,
}

/// `GET /admin/users`. The app reads it as [`UsersResponse`].
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminUsers {
  pub users: Vec<AdminUser>,
}

/// Whose a computer is, in the server admin's Devices.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionOwner {
  pub id: UserId,
  pub username: String,
  pub display_name: String,
}

/// A computer joined to the server (an app's device token), for admins.
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminSession {
  pub id: SessionId,
  pub device: String,
  pub user: SessionOwner,
  pub via: JoinedVia,
  pub created_at: i64,
  pub last_used_at: i64,
  /// When it ends unless it is used again: 90 days after its last use.
  pub expires_at: i64,
  /// The caller's own computer; never true for a browser signed in to the admin.
  pub current: bool,
}

/// `GET /admin/sessions`: newest use first.
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminSessions {
  pub sessions: Vec<AdminSession>,
}

/// A browser signed in to the server admin (an `admin_web` device token), for admins to sign out one by one.
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminBrowser {
  pub id: SessionId,
  pub user: SessionOwner,
  pub via: JoinedVia,
  pub created_at: i64,
  pub last_used_at: i64,
  /// 12 hours after the sign-in, however it is used.
  pub expires_at: i64,
  /// The browser asking.
  pub current: bool,
}

/// `GET /admin/browsers`: newest sign-in first.
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminBrowsers {
  pub browsers: Vec<AdminBrowser>,
}

/// API 2's `POST /admin/users`, removed in API 3 (accounts come from invites and codes). Kept while the frozen
/// `crates/desktop` names it through the workspace's refusing `create_user`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUser {
  pub username: String,
  pub display_name: String,
  pub admin: bool,
}

/// The answer to API 2's `POST /admin/users`; see [`CreateUser`].
#[derive(Debug, Serialize, Deserialize)]
pub struct UserCreated {
  pub user: User,
  pub initial_password: Credential,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserChanges {
  pub display_name: Option<String>,
  pub admin: Option<bool>,
  pub disabled: Option<bool>,
}

/// The answer to API 2's `POST /admin/users/{id}/password`, removed in API 3; kept like [`CreateUser`].
#[derive(Debug, Serialize, Deserialize)]
pub struct PasswordReset {
  pub initial_password: Credential,
}
