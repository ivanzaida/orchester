//! Orchester Cloud's JSON contract (`docs/architecture/server.md`, "Orchester Cloud"; `docs/cloud-api.md`): accounts,
//! devices and identity tokens. Used by the cloud, the desktop and the web front alike.

use orchester_core::id::{CloudAccountId, CloudDeviceId, InviteId, TeamServerId};
use serde::{Deserialize, Serialize};

use crate::{Credential, InviteKind, MemberRole, statements::InvitedProject};

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

/// `POST /auth/confirm/resend`: one more confirmation link for an unconfirmed account, carrying its newest sign-up's
/// password and display name, so confirming it still asks for the password chosen at sign-up.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResendConfirmation {
  pub email: String,
}

/// What an emailed link is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkPurpose {
  Confirm,
  Reset,
}

/// `POST /auth/links/inspect`: the address a live link is for, without using it, so its page can show it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkInspect {
  pub link_token: Credential,
  pub purpose: LinkPurpose,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LinkInspected {
  pub email: String,
}

/// `DELETE /me`: the account's password, asked again before everything the cloud holds about it is deleted.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountDeletion {
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

/// `202` of `POST /auth/sessions` for an account with two-factor authentication on: the password was right, and the
/// sign-in finishes with a code at `POST /auth/sessions/two-factor` before `expires_at` (milliseconds). The challenge
/// works once.
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudSignInChallenge {
  pub challenge: Credential,
  pub expires_at: i64,
}

/// `POST /auth/sessions/two-factor`: the challenge of the password step and a code from the authenticator app, or one
/// of the account's recovery codes.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TwoFactorSignIn {
  pub challenge: Credential,
  pub code: Credential,
}

/// `POST /me/two-factor`: starts turning two-factor authentication on, with the account's password.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TwoFactorStart {
  pub password: Credential,
}

/// `201` of `POST /me/two-factor`: the new secret in base32 and the `otpauth://` link an authenticator app reads from a
/// QR code. Shown while the setup is open; never again once it is confirmed.
#[derive(Debug, Serialize, Deserialize)]
pub struct TwoFactorSetup {
  pub secret: Credential,
  pub uri: Credential,
}

/// A code from the authenticator app (confirming the setup), or either kind of code (new recovery codes).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TwoFactorCode {
  pub code: Credential,
}

/// The account's ten recovery codes, shown once: each works once in place of a code from the app.
#[derive(Debug, Serialize, Deserialize)]
pub struct RecoveryCodes {
  pub recovery_codes: Vec<Credential>,
}

/// `DELETE /me/two-factor`: the password and a code from the app or a recovery code.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TwoFactorOff {
  pub password: Credential,
  pub code: Credential,
}

/// The account's two-factor authentication as `GET /me` reports it to the asking device.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwoFactorStatus {
  pub enabled: bool,
  /// Milliseconds since the Unix epoch.
  pub enabled_at: Option<i64>,
  /// Unused recovery codes; 0 while it is off.
  pub recovery_codes_left: u32,
  /// Whether the asking device signed in with a code, stepped up with one, or turned two-factor authentication on
  /// itself: only such a session accepts invites.
  pub this_session: bool,
}

/// `201` of `POST /auth/sessions/two-factor`: the signed-in device, as a password-only sign-in answers, and the
/// recovery codes left, so a client can warn when few remain.
#[derive(Debug, Serialize, Deserialize)]
pub struct TwoFactorSignedIn {
  #[serde(flatten)]
  pub signed_in: CloudSignedIn,
  pub recovery_codes_left: u32,
}

/// `200` of `POST /auth/sessions/current/two-factor`: the asking device is a two-factor session now.
#[derive(Debug, Serialize, Deserialize)]
pub struct SteppedUp {
  pub recovery_codes_left: u32,
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
  #[serde(default)]
  pub two_factor: TwoFactorStatus,
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
  /// `admin` for a sign-in to that server's admin, which must be linked to the cloud; `code` to show with a code
  /// entered in the app; `None` to join with a device.
  #[serde(default)]
  pub purpose: Option<TokenPurpose>,
  /// An open invite to the account's email from that server, which the token accepts: the token names it, and the
  /// team server admits the verified email to it. Only for a join (no purpose). The cloud's copy of the invite ends.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub invite: Option<InviteId>,
}

/// What an identity token is for besides joining a team server with a device (`docs/architecture/server.md`, "The
/// server admin"): the cloud names it in the token, and the team server accepts the token only for that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenPurpose {
  /// A sign-in to the team server's admin in the browser.
  Admin,
  /// Linking the team server to the cloud account of the admin who set it up.
  Link,
  /// Showing, with a code entered in the app, that the person signed in with two-factor authentication. The server
  /// takes it only with a code (`POST /auth/code`), never to join or sign in with the identity itself.
  Code,
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

/// A linked team server with its check code, for a page that asks the person to compare it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisteredServer {
  #[serde(flatten)]
  pub server: CloudServer,
  pub check_code: Option<String>,
}

/// `GET /servers/{id}`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerResponse {
  pub server: RegisteredServer,
}

/// `POST /servers`: the link statement the team server signed (`statements::LinkStatement`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerLink {
  pub link: String,
}

/// `201` of `POST /servers`: the server as registered, and what the server pins: the cloud's root key, the newest key
/// set it signed, and an identity token (purpose `link`) that links the admin's account there.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerLinked {
  pub server: RegisteredServer,
  pub root_key: String,
  pub key_set: String,
  pub identity_token: Credential,
  pub expires_at: i64,
}

/// A team server the account owns or joined (`GET /servers`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountServer {
  #[serde(flatten)]
  pub server: CloudServer,
  pub owner: bool,
  /// Milliseconds since the Unix epoch: when the account linked or first joined it.
  pub joined_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServersResponse {
  pub servers: Vec<AccountServer>,
}

/// `POST /invites`: an invite the team server signed (`statements::SignedInvite`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteDelivered {
  pub invite: String,
}

/// `202` of `POST /invites`: the server the invite is from, as the cloud registered it (its own address, for a page
/// that leads back to its admin).
#[derive(Debug, Serialize, Deserialize)]
pub struct InviteReceived {
  pub server: InvitingServer,
}

/// `DELETE /invites/{id}`: the team server's signed revocation (`statements::InviteRevocation`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteWithdrawn {
  pub revocation: String,
}

/// The team server an invite in flight is from.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InvitingServer {
  pub id: TeamServerId,
  pub name: String,
  pub address: String,
}

/// An open invite to the account's email (`GET /invites`). Times in milliseconds since the Unix epoch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudInvite {
  pub id: InviteId,
  pub kind: InviteKind,
  pub role: Option<MemberRole>,
  pub server: InvitingServer,
  pub project: Option<InvitedProject>,
  pub inviter_display_name: String,
  pub expires_at: i64,
  /// The asking device cannot accept it yet: accepting needs two-factor authentication on the account
  /// (`403 mfa_required`) and a device that signed in or stepped up with it (`403 session_without_mfa`).
  #[serde(default)]
  pub needs_two_factor: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloudInvites {
  pub invites: Vec<CloudInvite>,
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
  /// The team server is linked by a statement as new as this one, or this statement was used already.
  Conflict,
  /// The account owns as many linked team servers as one account may.
  TooManyServers,
  /// The cloud is at a capacity limit (its limiters' memory, its password hashing); try again shortly.
  Busy,
  /// Accepting an invite needs two-factor authentication on the account, which is off.
  MfaRequired,
  /// The account has two-factor authentication, but the asking device signed in without it: step up with a code, or
  /// sign in again.
  SessionWithoutMfa,
  /// A two-factor code or recovery code that is wrong or was used.
  InvalidCode,
  /// A sign-in's two-factor challenge that is unknown, used, expired or out of tries: sign in again.
  ChallengeExpired,
  Internal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloudError {
  pub error: CloudErrorCode,
  /// English diagnostics; the desktop and the web front show their own text by code.
  pub message: String,
  /// Why a device's or a web session's token ended, with `unauthorized` for a token the cloud ended itself.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reason: Option<SessionEnd>,
  /// When that token ended, in milliseconds, with `reason`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub ended_at: Option<i64>,
}

/// Why the cloud ended a device's token (`docs/cloud-api.md`, "Why a token ended"). A client that meets a reason it
/// does not know reads [`Self::Other`] and shows its general words.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionEnd {
  /// Signed out from another device of the account, or from Devices on the cloud's account page.
  Revoked,
  /// The account's password was reset, or changed on another device.
  PasswordReset,
  /// The account was deleted.
  AccountDeleted,
  /// The device signed itself out.
  SignedOut,
  #[serde(other)]
  Other,
}
