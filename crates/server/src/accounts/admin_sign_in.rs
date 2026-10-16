//! Signing in to the server admin in a browser (`docs/architecture/server.md`, "The server admin"): with a cloud
//! identity token issued for that purpose, or a setup, pairing or account code. The browser gets a device token of the
//! kind `admin_web` (12 hours, only what the admin pages call), and only for an account that has something to manage
//! there: an admin, or an owner of a project. Join approval does not hold it: it adds no computer.

use std::net::IpAddr;

use orchester_api::{CloudJoin, DeviceKind, ErrorCode, Joined, JoinedVia, SetupCheck, User};
use rusqlite::Connection;

use super::Server;
use crate::{
  approvals,
  cloud_link::{self, Claims},
  codes::{self, CodeKind},
  db,
  devices::{self, Issued},
  error::ServerError,
  rate_limit::JoinKind,
};

impl Server {
  /// `POST /auth/setup`: whether `code` is the open setup code of a server with no admin yet, without using it up.
  /// A wrong code counts like a failed redemption.
  pub(crate) async fn check_setup_code(&self, input: SetupCheck, address: IpAddr) -> Result<(), ServerError> {
    let attempt = self.inner.limits.begin(JoinKind::Code, address, self.now())?;
    let state = self.inner.clone();
    let found = self
      .snapshot(move |db, now| {
        let code = codes::find(db, &state.keys, input.code.expose(), now)?;
        Ok(code.is_some_and(|code| code.kind == CodeKind::Setup) && !db::has_admin(db)?)
      })
      .await?;
    if found {
      attempt.succeeded();
      return Ok(());
    }
    attempt.failed(self.now());
    self
      .account_write(move |db, now| db::failed_join(db, "code", address, now))
      .await?;
    Err(ErrorCode::InvalidCode.into())
  }
}

/// Whether `user` has anything to manage in the server admin: an admin, or an owner of a project.
pub(crate) fn manages(db: &Connection, user: &User) -> Result<bool, ServerError> {
  if user.admin {
    return Ok(true);
  }
  Ok(db.query_row(
    "SELECT EXISTS(SELECT 1 FROM memberships WHERE user_id=?1 AND role='owner')",
    [user.id.to_string()],
    |row| row.get(0),
  )?)
}

/// A browser's admin sign-in for `user`, refused with `403 nothing_to_manage` before anything is used up when the
/// account has nothing to manage.
pub(crate) fn admin_session(
  db: &Connection,
  user: User,
  device: &str,
  via: JoinedVia,
  kid: Option<&str>,
  now: i64,
) -> Result<Joined, ServerError> {
  if !manages(db, &user)? {
    return Err(ErrorCode::NothingToManage.into());
  }
  let issued = Issued {
    device,
    kind: DeviceKind::AdminWeb,
    via,
    kid,
  };
  let (token, session) = devices::issue(db, &user, &issued, now)?;
  Ok(Joined {
    token,
    session,
    user,
    joined: None,
  })
}

/// A cloud sign-in to the admin: the account linked to the token's cloud identity, with a token issued for the admin.
/// While join approval is on (row 57) it is refused with `403 approval_required`, before anything is used up: a
/// cloud identity alone must not open the admin when it cannot add a computer without approval. A pairing code made
/// on one of the account's own computers, or an account code from the host, signs the browser in instead.
pub(crate) fn with_cloud(
  db: &Connection,
  input: &CloudJoin,
  claims: &Claims,
  address: IpAddr,
  now: i64,
) -> Result<Joined, ServerError> {
  let user = db::user_by_subject(db, &claims.sub)?.ok_or(ErrorCode::NotInvited)?;
  if user.disabled {
    return Err(ErrorCode::Forbidden.into());
  }
  if approvals::enabled(db)? {
    return Err(ErrorCode::ApprovalRequired.into());
  }
  let joined = admin_session(db, user, &input.device, JoinedVia::Cloud, Some(&claims.kid), now)?;
  cloud_link::consume(db, claims)?;
  db::audit(
    db,
    Some(joined.user.id),
    "admin_signed_in",
    &format!("cloud:{}:{}", claims.kid, claims.sub),
    Some(address),
    now,
  )?;
  Ok(joined)
}

/// Whether a code of `kind` may sign a browser in to the admin: setup, pairing and account codes may; an invite code
/// is for the app, and is refused like a wrong code, unused.
pub(crate) fn code_signs_in(kind: CodeKind) -> bool {
  kind != CodeKind::Invite
}
