use orchester_core::id::{SessionId, UserId};
use serde::{Deserialize, Serialize};

use crate::Credential;

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
  pub name: String,
  pub api_version: u8,
  pub needs_bootstrap: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
  pub id: UserId,
  pub username: String,
  pub display_name: String,
  pub admin: bool,
  pub disabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
  pub id: SessionId,
  pub device: String,
  pub created_at: i64,
  pub last_used_at: i64,
  pub current: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignIn {
  pub username: String,
  pub password: Credential,
  pub device: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedIn {
  pub token: Credential,
  pub session: Session,
  pub user: User,
  pub password_change_required: bool,
}

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUser {
  pub username: String,
  pub display_name: String,
  pub admin: bool,
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct PasswordReset {
  pub initial_password: Credential,
}
