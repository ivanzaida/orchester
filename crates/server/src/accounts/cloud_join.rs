//! Joining with an Orchester Cloud identity token (`POST /auth/cloud`), verified here without the internet: the
//! account linked to the token's cloud account, or an open invite for its verified email, or a refusal. With a device
//! token, the request links the caller's code account to the identity instead.

use std::net::IpAddr;

use orchester_api::{Approval, CloudJoin, ErrorCode, InviteDelivery, InviteKind, Joined, JoinedVia, User};
use orchester_core::id::ApprovalId;
use rusqlite::Connection;

use super::Server;
use crate::{
  approvals,
  cloud_link::{self, Link, identity::Claims},
  db::{self, Authenticated, NewUser},
  devices,
  error::ServerError,
  invites,
  rate_limit::JoinKind,
};

/// What a cloud join came to.
pub(crate) enum CloudJoined {
  Joined(Joined),
  /// Join approval is on: the join waits for the account's devices or an admin.
  Waiting(Approval),
  /// The caller's account is now linked to the identity.
  Linked(User),
}

impl Server {
  pub(crate) async fn join_with_cloud(
    &self,
    input: CloudJoin,
    caller: Option<Authenticated>,
    address: IpAddr,
  ) -> Result<CloudJoined, ServerError> {
    devices::check_device(&input.device)?;
    let attempt = self.inner.limits.begin(JoinKind::Cloud, address, self.now())?;
    let state = self.inner.clone();
    let result = self
      .account_write(move |db, now| {
        let mut link = Link::load(db)?.ok_or(ErrorCode::CloudOff)?;
        let claims = link.verify(
          db,
          &state.keys,
          input.identity_token.expose(),
          input.key_set.as_deref(),
          now,
        )?;
        if let Some(caller) = caller {
          return link_caller(db, &caller, &claims, now).map(CloudJoined::Linked);
        }
        join(db, &input, &claims, address, now)
      })
      .await;
    match result {
      Err(ServerError::Api(code @ (ErrorCode::InvalidIdentity | ErrorCode::NotInvited))) => {
        attempt.failed(self.now());
        self
          .account_write(move |db, now| db::audit(db, None, "join_failed", "cloud", Some(address), now))
          .await?;
        Err(code.into())
      }
      other => {
        attempt.succeeded();
        other
      }
    }
  }
}

impl Server {
  pub(crate) async fn approvals(&self, auth: Authenticated) -> Result<Vec<Approval>, ServerError> {
    self
      .snapshot(move |db, now| {
        let current = db::checked(db, &auth, now, false)?;
        approvals::waiting(db, &current.user, now)
      })
      .await
  }

  pub(crate) async fn decide_approval(
    &self,
    auth: Authenticated,
    id: ApprovalId,
    approve: bool,
  ) -> Result<Approval, ServerError> {
    self
      .account_write(move |db, now| {
        let current = db::checked(db, &auth, now, false)?;
        approvals::decide(db, &current.user, id, approve, now)
      })
      .await
  }
}

fn join(
  db: &Connection,
  input: &CloudJoin,
  claims: &Claims,
  address: IpAddr,
  now: i64,
) -> Result<CloudJoined, ServerError> {
  let invite = claims
    .invite
    .as_deref()
    .and_then(|id| id.parse().ok())
    .map(|id| invites::open(db, id, now))
    .transpose()?
    .flatten()
    .filter(|invite| {
      invite.delivery == InviteDelivery::Cloud
        && invite.email.as_deref() == Some(claims.email.to_ascii_lowercase().as_str())
    });
  let (user, joined) = match db::user_by_subject(db, &claims.sub)? {
    Some(user) => {
      if user.disabled {
        return Err(ErrorCode::Forbidden.into());
      }
      if approvals::enabled(db)? {
        let waiting = match input.approval {
          None => Some(approvals::request(db, user.id, &claims.sub, &input.device, now)?),
          Some(id) => approvals::finish(db, id, user.id, &claims.sub, now)?,
        };
        if let Some(waiting) = waiting {
          return Ok(CloudJoined::Waiting(waiting));
        }
      }
      let joined = invite
        .map(|invite| invites::accept(db, &invite, user.id, now))
        .transpose()?
        .flatten();
      (user, joined)
    }
    None => {
      let invite = invite.ok_or(ErrorCode::NotInvited)?;
      let user = invited_account(db, &invite, claims, now)?;
      let joined = invites::accept(db, &invite, user.id, now)?;
      (user, joined)
    }
  };
  cloud_link::consume(db, claims)?;
  let (token, session) = devices::issue(db, &user, &input.device, JoinedVia::Cloud, Some(&claims.kid), now)?;
  db::audit(
    db,
    Some(user.id),
    "joined",
    &format!("cloud:{}:{}", claims.kid, claims.sub),
    Some(address),
    now,
  )?;
  Ok(CloudJoined::Joined(Joined {
    token,
    session,
    user,
    joined,
  }))
}

/// The account an invite gives a cloud identity the server does not know yet: the existing account an account invite
/// names, or a new one named after the identity.
fn invited_account(db: &Connection, invite: &invites::Stored, claims: &Claims, now: i64) -> Result<User, ServerError> {
  if invite.kind == InviteKind::Account {
    let user = invite
      .user
      .map(|id| db::user_by_id(db, id))
      .transpose()?
      .flatten()
      .filter(|user| !user.disabled && !user.cloud)
      .ok_or(ErrorCode::NotInvited)?;
    db::link_subject(db, user.id, &claims.sub, now)?;
    return Ok(User { cloud: true, ..user });
  }
  let username = free_username(db, claims)?;
  db::create_user(
    db,
    &NewUser {
      username: &username,
      display_name: &claims.name,
      admin: invite.admin,
      cloud_subject: Some(&claims.sub),
    },
    None,
    now,
  )
}

/// Links the caller's account, which has no cloud identity yet, to the token's.
fn link_caller(db: &Connection, caller: &Authenticated, claims: &Claims, now: i64) -> Result<User, ServerError> {
  let current = db::checked(db, caller, now, false)?;
  if current.user.cloud || db::user_by_subject(db, &claims.sub)?.is_some() {
    return Err(ErrorCode::Conflict.into());
  }
  db::link_subject(db, current.user.id, &claims.sub, now)?;
  cloud_link::consume(db, claims)?;
  Ok(User {
    cloud: true,
    ..current.user
  })
}

/// A username from the identity's display name or email, unique on the server (`alice`, `alice-2`, …).
fn free_username(db: &Connection, claims: &Claims) -> Result<String, ServerError> {
  let local = claims.email.split('@').next().unwrap_or_default();
  let base: String = [claims.name.as_str(), local]
    .into_iter()
    .map(|source| {
      source
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .filter(|c| c.is_ascii_alphanumeric() || "._-".contains(*c))
        .take(56)
        .collect::<String>()
    })
    .find(|candidate| !candidate.is_empty())
    .unwrap_or_else(|| "user".into());
  let mut candidate = base.clone();
  let mut suffix = 1_u32;
  while db::find_user(db, &candidate)?.is_some() {
    suffix += 1;
    candidate = format!("{base}-{suffix}");
  }
  Ok(candidate)
}
