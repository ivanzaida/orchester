//! Joining with a code (`POST /auth/code`) and making pairing codes (`POST /auth/pairings`).

use std::net::IpAddr;

use orchester_api::{CodeJoin, ErrorCode, Joined, JoinedVia, PairingCode, User};
use rusqlite::Connection;

use super::Server;
use crate::{
  codes::{self, CodeKind, NewCode, OpenCode},
  db::{self, Authenticated, NewUser},
  devices,
  error::ServerError,
  invites,
  rate_limit::JoinKind,
};

impl Server {
  pub(crate) async fn join_with_code(&self, input: CodeJoin, address: IpAddr) -> Result<Joined, ServerError> {
    devices::check_device(&input.device)?;
    let attempt = self.inner.limits.begin(JoinKind::Code, address, self.now())?;
    let state = self.inner.clone();
    let result = self
      .account_write(move |db, now| {
        let code = codes::find(db, &state.keys, input.code.expose(), now)?.ok_or(ErrorCode::InvalidCode)?;
        let (user, joined) = redeemed_account(db, &code, &input, now)?;
        codes::redeem(db, &code, now)?;
        let (token, session) = devices::issue(db, &user, &input.device, JoinedVia::Code, None, now)?;
        db::audit(
          db,
          Some(user.id),
          "joined",
          &format!("code:{}", code.kind.as_str()),
          Some(address),
          now,
        )?;
        Ok(Joined {
          token,
          session,
          user,
          joined,
        })
      })
      .await;
    match result {
      Err(ServerError::Api(ErrorCode::InvalidCode)) => {
        let now = self.now();
        attempt.failed(now);
        self
          .account_write(move |db, now| db::audit(db, None, "join_failed", "code", Some(address), now))
          .await?;
        Err(ErrorCode::InvalidCode.into())
      }
      other => {
        attempt.succeeded();
        other
      }
    }
  }

  pub(crate) async fn pairing_code(&self, auth: Authenticated) -> Result<PairingCode, ServerError> {
    let state = self.inner.clone();
    self
      .account_write(move |db, now| {
        let current = db::checked(db, &auth, now, false)?;
        let new = NewCode {
          kind: CodeKind::Pairing,
          invite: None,
          user: Some(current.user.id),
          made_by: Some(current.user.id),
        };
        let (code, expires_at) = codes::create(db, &state.keys, &new, now)?;
        db::audit(db, Some(current.user.id), "code_made", "pairing", None, now)?;
        Ok(PairingCode { code, expires_at })
      })
      .await
  }
}

/// The account a code gives, made now for setup and invite codes, with the project membership an invite grants.
fn redeemed_account(
  db: &Connection,
  code: &OpenCode,
  input: &CodeJoin,
  now: i64,
) -> Result<(User, Option<orchester_api::JoinedProject>), ServerError> {
  match code.kind {
    CodeKind::Setup => {
      if db::has_admin(db)? {
        return Err(ErrorCode::InvalidCode.into());
      }
      Ok((new_account(db, input, true, now)?, None))
    }
    CodeKind::Invite => {
      let invite = code
        .invite
        .map(|id| invites::open(db, id, now))
        .transpose()?
        .flatten()
        .ok_or(ErrorCode::InvalidCode)?;
      let user = new_account(db, input, invite.admin, now)?;
      let joined = invites::accept(db, &invite, user.id, now)?;
      Ok((user, joined))
    }
    CodeKind::Account | CodeKind::Pairing => {
      let user = code
        .user
        .map(|id| db::user_by_id(db, id))
        .transpose()?
        .flatten()
        .filter(|user| !user.disabled)
        .ok_or(ErrorCode::InvalidCode)?;
      if let Some(invite) = code.invite.map(|id| invites::open(db, id, now)).transpose()?.flatten() {
        invites::accept(db, &invite, user.id, now)?;
      }
      Ok((user, None))
    }
  }
}

fn new_account(db: &Connection, input: &CodeJoin, admin: bool, now: i64) -> Result<User, ServerError> {
  let (Some(username), Some(display_name)) = (&input.username, &input.display_name) else {
    return Err(ErrorCode::Invalid.into());
  };
  db::create_user(
    db,
    &NewUser {
      username,
      display_name,
      admin,
      cloud_subject: None,
    },
    None,
    now,
  )
}
