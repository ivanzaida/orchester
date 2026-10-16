//! Joining with an Orchester Cloud identity token (`POST /auth/cloud`), verified here without the internet: the
//! account linked to the token's cloud account, or an open invite for its verified email, or a refusal. With a device
//! token, the request links the caller's code account to the identity instead.

use std::net::IpAddr;

use orchester_api::{
  Approval, CloudJoin, DeviceKind, ErrorCode, InviteDelivery, InviteKind, Joined, JoinedProject, JoinedVia, User,
  cloud::TokenPurpose,
};
use orchester_core::id::ApprovalId;
use rusqlite::Connection;

use super::{Server, admin_sign_in};
use crate::{
  approvals,
  cloud_link::{self, Claims, Link},
  db::{self, Authenticated, NewUser},
  devices::{self, Issued},
  error::ServerError,
  invites,
  rate_limit::JoinKind,
};

/// What a cloud join came to.
pub(crate) enum CloudJoined {
  Joined(Joined),
  /// Join approval is on: the join waits for the account's devices or an admin; `new` when this join asked first.
  Waiting {
    approval: Approval,
    new: bool,
  },
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
        if input.kind == DeviceKind::AdminWeb {
          claims.require(Some(TokenPurpose::Admin))?;
          return admin_sign_in::with_cloud(db, &input, &claims, address, now).map(CloudJoined::Joined);
        }
        claims.require(None)?;
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
          .account_write(move |db, now| db::failed_join(db, "cloud", address, now))
          .await?;
        Err(code.into())
      }
      // A new approval counts against the address like a failure: asking costs the approvers' attention.
      Ok(waiting @ CloudJoined::Waiting { new: true, .. }) => {
        attempt.failed(self.now());
        Ok(waiting)
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
        approvals::waiting(db, &current, now)
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
        approvals::decide(db, &current, id, approve, now)
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
    // An invite id this server could not have made names none of its invites: the join goes on as without one.
    .and_then(|id| id.parse().ok())
    .map(|id| invites::open(db, id, now))
    .transpose()?
    .flatten()
    .filter(|invite| {
      invite.delivery == InviteDelivery::Cloud
        && invite.email.as_deref() == Some(claims.email.to_ascii_lowercase().as_str())
    });
  let (user, joined) = match db::user_by_subject(db, &claims.sub)? {
    Some(user) => match known_account(
      db,
      &Asked {
        input,
        claims,
        address,
        now,
      },
      invite,
      user,
    )? {
      Known::Joins(user, joined) => (user, joined),
      Known::Waits(waiting) => return Ok(waiting),
    },
    None => {
      let invite = invite.ok_or(ErrorCode::NotInvited)?;
      let user = invited_account(db, &invite, claims, now)?;
      let joined = invites::accept(db, &invite, user.id, now)?;
      (user, joined)
    }
  };
  cloud_link::consume(db, claims)?;
  let issued = Issued {
    device: &input.device,
    kind: DeviceKind::App,
    via: JoinedVia::Cloud,
    kid: Some(&claims.kid),
  };
  let (token, session) = devices::issue(db, &user, &issued, now)?;
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

/// A cloud join being decided: what the app sent, what its token says, from where and when.
struct Asked<'a> {
  input: &'a CloudJoin,
  claims: &'a Claims,
  address: IpAddr,
  now: i64,
}

enum Known {
  Joins(User, Option<JoinedProject>),
  Waits(CloudJoined),
}

/// What a cloud join of the account linked to the identity comes to: the account, with what an invite of it grants
/// (an admin invite makes it an admin), or the approval it waits for. An account invite names one account: the
/// identity of another is refused, and the invite stays open.
fn known_account(
  db: &Connection,
  asked: &Asked<'_>,
  invite: Option<invites::Stored>,
  user: User,
) -> Result<Known, ServerError> {
  let Asked {
    input,
    claims,
    address,
    now,
  } = *asked;
  if user.disabled {
    return Err(ErrorCode::Forbidden.into());
  }
  if invite
    .as_ref()
    .is_some_and(|invite| invite.kind == InviteKind::Account && invite.user != Some(user.id))
  {
    return Err(ErrorCode::NotInvited.into());
  }
  if approvals::enabled(db)? {
    let waiting = match input.approval {
      None => Some(approvals::request(
        db,
        user.id,
        &claims.sub,
        &input.device,
        address,
        now,
      )?),
      Some(id) => approvals::finish(db, id, user.id, &claims.sub, now)?.map(|approval| (approval, false)),
    };
    if let Some((approval, new)) = waiting {
      return Ok(Known::Waits(CloudJoined::Waiting { approval, new }));
    }
  }
  let Some(invite) = invite else {
    return Ok(Known::Joins(user, None));
  };
  let joined = invites::accept(db, &invite, user.id, now)?;
  let user = db::user_by_id(db, user.id)?.ok_or(ServerError::StoredData)?;
  Ok(Known::Joins(user, joined))
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
  // An admin's browser links the account through `PATCH /server/cloud`, with a token issued for that.
  if current.kind == DeviceKind::AdminWeb {
    return Err(ErrorCode::Forbidden.into());
  }
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
