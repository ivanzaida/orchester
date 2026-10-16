//! Joining with a code (`POST /auth/code`) and making pairing codes (`POST /auth/pairings`). Every code but the setup
//! code, entered in the app, needs the app's Orchester Cloud identity token with the two-factor claim (`mfa`), verified
//! here offline against the pinned root and the key set the app carries (`docs/architecture/server.md`, "Codes").

use std::net::IpAddr;

use orchester_api::{CodeJoin, DeviceKind, ErrorCode, Joined, JoinedVia, PairingCode, User};
use rusqlite::Connection;

use super::{Server, admin_sign_in};
use crate::{
  cloud_link::{self, Claims, Link},
  codes::{self, CodeKind, NewCode, OpenCode},
  db::{self, Authenticated, NewUser},
  devices::{self, Issued},
  error::ServerError,
  invites,
  rate_limit::JoinKind,
  server_keys::ServerKeys,
};

impl Server {
  pub(crate) async fn join_with_code(&self, input: CodeJoin, address: IpAddr) -> Result<Joined, ServerError> {
    devices::check_device(&input.device)?;
    let attempt = self.inner.limits.begin(JoinKind::Code, address, self.now())?;
    let state = self.inner.clone();
    let result = self
      .account_write(move |db, now| {
        let code = codes::find(db, &state.keys, input.code.expose(), now)?
          .filter(|code| input.kind == DeviceKind::App || admin_sign_in::code_signs_in(code.kind))
          .ok_or(ErrorCode::InvalidCode)?;
        let proof = (input.kind == DeviceKind::App && code.kind != CodeKind::Setup)
          .then(|| two_factor_proof(db, &state.keys, &input, now))
          .transpose()?;
        let (user, joined) = redeemed_account(db, &code, &input, now)?;
        let joined = issue(db, (&input, &code, proof.as_ref()), user, joined, now)?;
        codes::redeem(db, &code, now)?;
        if let Some(claims) = &proof {
          cloud_link::consume(db, claims)?;
        }
        let action = if input.kind == DeviceKind::AdminWeb {
          "admin_signed_in"
        } else {
          "joined"
        };
        let target = match &proof {
          Some(claims) => format!("code:{}:mfa:{}", code.kind.as_str(), claims.sub),
          None => format!("code:{}", code.kind.as_str()),
        };
        db::audit(db, Some(joined.user.id), action, &target, Some(address), now)?;
        Ok(joined)
      })
      .await;
    match result {
      Err(ServerError::Api(code @ (ErrorCode::InvalidCode | ErrorCode::InvalidIdentity))) => {
        let now = self.now();
        attempt.failed(now);
        self
          .account_write(move |db, now| db::failed_join(db, "code", address, now))
          .await?;
        Err(code.into())
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
          mfa: current.mfa,
        };
        let (code, expires_at) = codes::create(db, &state.keys, &new, now)?;
        db::audit(db, Some(current.user.id), "code_made", "pairing", None, now)?;
        Ok(PairingCode { code, expires_at })
      })
      .await
  }
}

/// The cloud identity a code join in the app shows: a token for this server, to join with (no purpose), whose device
/// signed in with two-factor authentication. Verified offline whether or not the server takes cloud identities to join
/// with. A server with no root pinned cannot check it (`403 mfa_unverifiable`); a token without the claim, or none,
/// is `403 mfa_required`; anything the root does not vouch for is `401 invalid_identity`. Nothing is used up first.
fn two_factor_proof(db: &Connection, keys: &ServerKeys, input: &CodeJoin, now: i64) -> Result<Claims, ServerError> {
  let mut link = Link::load(db)?.ok_or(ErrorCode::MfaUnverifiable)?;
  let token = input.identity_token.as_ref().ok_or(ErrorCode::MfaRequired)?;
  let claims = link.check(db, keys, token.expose(), input.key_set.as_deref(), now)?;
  claims.require(None)?;
  if !claims.mfa {
    return Err(ErrorCode::MfaRequired.into());
  }
  Ok(claims)
}

/// The device token a code join issues: a browser's admin sign-in, or an app's device, each keeping whether it came
/// with two-factor authentication.
fn issue(
  db: &Connection,
  (input, code, proof): (&CodeJoin, &OpenCode, Option<&Claims>),
  user: User,
  joined: Option<orchester_api::JoinedProject>,
  now: i64,
) -> Result<Joined, ServerError> {
  if input.kind == DeviceKind::AdminWeb {
    return admin_sign_in::admin_session(db, user, &input.device, (None, code.mfa), now);
  }
  let issued = Issued {
    device: &input.device,
    kind: DeviceKind::App,
    via: JoinedVia::Code,
    kid: proof.map(|claims| claims.kid.as_str()),
    mfa: proof.is_some_and(|claims| claims.mfa),
  };
  let (token, session) = devices::issue(db, &user, &issued, now)?;
  Ok(Joined {
    token,
    session,
    user,
    joined,
  })
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
      invites::still_allowed(db, &invite)?;
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
