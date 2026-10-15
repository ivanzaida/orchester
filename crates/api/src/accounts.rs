use orchester_core::id::{SessionId, UserId};
use serde::{Deserialize, Serialize};

use crate::Credential;

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
  pub name: String,
  pub api_version: u8,
  /// `needs_setup` on the wire since API 3: the server has no admin yet and prints a setup code. The Rust name is
  /// API 2's while the frozen `crates/desktop` reads it.
  #[serde(rename = "needs_setup")]
  pub needs_bootstrap: bool,
  /// The cloud link, `null` when the server is not linked to Orchester Cloud.
  pub cloud: Option<CloudStatus>,
}

/// What `GET /server` says about the cloud link, before anyone signs in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudStatus {
  /// The base64url SHA-256 of the cloud's root key the server pinned.
  pub root_fingerprint: String,
  /// Whether the server accepts identity tokens now (the owner can turn it off).
  pub accepting: bool,
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
