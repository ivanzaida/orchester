//! The server's link to Orchester Cloud (`docs/architecture/server.md`, "Identity tokens and keys"): the cloud's root
//! key the server pinned, the newest key set that root signed, and whether cloud identities are accepted. Tokens are
//! verified here, without the internet; the owner can turn the cloud off, re-pin its root or unlink it at once.

use orchester_api::{
  CloudLink, CloudStatus, ErrorCode, UserRef,
  cloud::TokenPurpose,
  identity::{CLOCK_SKEW_SECS, IdentityError, PublicKey, VerifiedKeySet, VerifiedToken},
  statements::{LINK_STATEMENT_TYPE, LinkStatement},
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{error::ServerError, server_keys::ServerKeys};

/// What a verified identity token says that the server uses, with the id of the cloud key that signed it (for the
/// audit and the device token's record).
pub(crate) struct Claims {
  /// The cloud account.
  pub sub: String,
  /// The account's confirmed email.
  pub email: String,
  pub name: String,
  /// The invite the token was issued for, when it came from accepting one.
  pub invite: Option<String>,
  /// `None` to join with a device; otherwise the one use the cloud issued the token for.
  pub purpose: Option<TokenPurpose>,
  /// The cloud device signed in with two-factor authentication: a device token issued for it may send invites.
  pub mfa: bool,
  pub jti: String,
  pub exp: i64,
  pub kid: String,
}

/// The longest name, in characters, a cloud identity may give a new account.
const MAX_NAME_CHARS: usize = 256;

impl Claims {
  /// A token issued for another use than `purpose` is refused like any token the server cannot use.
  pub fn require(&self, purpose: Option<TokenPurpose>) -> Result<(), ServerError> {
    if self.purpose != purpose {
      return Err(ErrorCode::InvalidIdentity.into());
    }
    Ok(())
  }

  /// The claims of a verified token; a blank or overlong name is refused like any token the server cannot use.
  fn of(verified: VerifiedToken) -> Result<Self, ServerError> {
    let VerifiedToken { claims, kid } = verified;
    if claims.name.trim().is_empty() || claims.name.chars().count() > MAX_NAME_CHARS {
      return Err(ErrorCode::InvalidIdentity.into());
    }
    Ok(Self {
      sub: claims.sub.to_string(),
      email: claims.email,
      name: claims.name,
      invite: claims.invite,
      purpose: claims.purpose,
      mfa: claims.mfa,
      jti: claims.jti,
      exp: claims.exp,
      kid,
    })
  }
}

/// The link as stored.
pub(crate) struct Link {
  root_key: PublicKey,
  key_set: Option<String>,
  key_set_issued_at: Option<i64>,
  key_set_expires_at: Option<i64>,
  accepting: bool,
  linked_at: i64,
  linked_by: Option<UserRef>,
}

impl Link {
  pub fn load(db: &Connection) -> Result<Option<Self>, ServerError> {
    let row = db
      .query_row(
        "SELECT root_key,key_set,key_set_issued_at,key_set_expires_at,accepting,linked_at,linked_by FROM cloud_link
          WHERE id=1",
        [],
        |row| {
          Ok((
            (row.get::<_, String>(0)?, row.get(1)?, row.get(2)?, row.get(3)?),
            (row.get(4)?, row.get(5)?, row.get::<_, Option<String>>(6)?),
          ))
        },
      )
      .optional()?;
    row
      .map(
        |((root, key_set, issued, expires), (accepting, linked_at, linked_by))| {
          let linked_by = linked_by
            .map(|id| id.parse().map_err(|_| ServerError::StoredData))
            .transpose()?
            .map(|id| crate::sharing::access::user_ref(db, id))
            .transpose()?;
          Ok(Self {
            root_key: PublicKey::parse(&root).map_err(|_| ServerError::StoredData)?,
            key_set,
            key_set_issued_at: issued,
            key_set_expires_at: expires,
            accepting,
            linked_at,
            linked_by,
          })
        },
      )
      .transpose()
  }

  pub fn status(&self, keys: &ServerKeys) -> CloudStatus {
    CloudStatus {
      server: keys.id.to_string(),
      root_fingerprint: self.root_key.fingerprint(),
      accepting: self.accepting,
      codes_need_two_factor: true,
    }
  }

  /// Verifies an identity token for this server. A key set the app carries replaces the cached one when the root
  /// signed it and it is newer; one that does not verify is ignored.
  pub fn verify(
    &mut self,
    db: &Connection,
    keys: &ServerKeys,
    token: &str,
    offered: Option<&str>,
    now_ms: i64,
  ) -> Result<Claims, ServerError> {
    if !self.accepting {
      return Err(ErrorCode::CloudOff.into());
    }
    self.check(db, keys, token, offered, now_ms)
  }

  /// Verifies an identity token as [`Self::verify`] does, whether or not the server accepts cloud identities to join
  /// with: what a code join in the app shows with it is only that its person signed in with two-factor authentication.
  pub fn check(
    &mut self,
    db: &Connection,
    keys: &ServerKeys,
    token: &str,
    offered: Option<&str>,
    now_ms: i64,
  ) -> Result<Claims, ServerError> {
    let now = now_ms.div_euclid(1000);
    if let Some(offered) = offered {
      self.offer(db, offered, now)?;
    }
    let set = self.cached(now)?;
    let claims = Claims::of(set.verify_token(token, &keys.id.to_string(), now).map_err(refusal)?)?;
    db.execute("DELETE FROM used_identity_tokens WHERE expires_at<?1", [now])?;
    let used: bool = db.query_row(
      "SELECT EXISTS(SELECT 1 FROM used_identity_tokens WHERE jti=?1)",
      [&claims.jti],
      |row| row.get(0),
    )?;
    if used {
      return Err(ErrorCode::InvalidIdentity.into());
    }
    Ok(claims)
  }

  fn offer(&mut self, db: &Connection, offered: &str, now: i64) -> Result<(), ServerError> {
    let Ok(set) = VerifiedKeySet::verify(offered, &self.root_key, now) else {
      return Ok(());
    };
    if self
      .key_set_issued_at
      .is_some_and(|issued| issued >= set.statement().issued_at)
    {
      return Ok(());
    }
    self.cache(db, offered, &set)
  }

  fn cache(&mut self, db: &Connection, text: &str, set: &VerifiedKeySet) -> Result<(), ServerError> {
    let (issued_at, expires_at) = (set.statement().issued_at, set.statement().expires_at);
    db.execute(
      "UPDATE cloud_link SET key_set=?1,key_set_issued_at=?2,key_set_expires_at=?3 WHERE id=1",
      params![text, issued_at, expires_at],
    )?;
    self.key_set = Some(text.to_owned());
    self.key_set_issued_at = Some(issued_at);
    self.key_set_expires_at = Some(expires_at);
    Ok(())
  }

  /// The cached key set while it has not expired; `409 key_set_stale` otherwise.
  fn cached(&self, now: i64) -> Result<VerifiedKeySet, ServerError> {
    let text = self.key_set.as_deref().ok_or(ErrorCode::KeySetStale)?;
    VerifiedKeySet::verify(text, &self.root_key, now).map_err(|error| match error {
      IdentityError::KeySetExpired | IdentityError::NotYetValid => ErrorCode::KeySetStale.into(),
      _ => ServerError::StoredData,
    })
  }

  pub fn describe(link: Option<&Self>, keys: &ServerKeys) -> CloudLink {
    CloudLink {
      server: keys.id.to_string(),
      root_fingerprint: link.map(|link| link.root_key.fingerprint()),
      accepting: link.is_some_and(|link| link.accepting),
      key_set_expires_at: link
        .and_then(|link| link.key_set_expires_at)
        .map(|seconds| seconds * 1000),
      key_set_issued_at: link
        .and_then(|link| link.key_set_issued_at)
        .map(|seconds| seconds * 1000),
      linked_at: link.map(|link| link.linked_at),
      linked_by: link.and_then(|link| link.linked_by.clone()),
    }
  }
}

/// Pins `root_key` (base64url) and caches `key_set`, which that root must have signed: the end of linking, by the admin
/// `by`.
pub(crate) fn link(
  db: &Connection,
  root_key: &str,
  key_set: &str,
  by: orchester_core::id::UserId,
  now_ms: i64,
) -> Result<Link, ServerError> {
  let root = PublicKey::parse(root_key).map_err(|_| ErrorCode::Invalid)?;
  let set = VerifiedKeySet::verify(key_set, &root, now_ms.div_euclid(1000)).map_err(|_| ErrorCode::Invalid)?;
  let (issued_at, expires_at) = (set.statement().issued_at, set.statement().expires_at);
  db.execute(
    "INSERT INTO cloud_link(id,root_key,key_set,key_set_issued_at,key_set_expires_at,accepting,linked_at,linked_by)
      VALUES (1,?1,?2,?3,?4,1,?5,?6)
      ON CONFLICT(id) DO UPDATE SET root_key=?1,key_set=?2,key_set_issued_at=?3,key_set_expires_at=?4,accepting=1,
        linked_at=?5,linked_by=?6",
    params![root.encoded(), key_set, issued_at, expires_at, now_ms, by.to_string()],
  )?;
  Link::load(db)?.ok_or(ServerError::StoredData)
}

/// Replaces the pinned root (after a root compromise). The cached key set, signed by the old root, goes with it;
/// cloud joins wait for a key set the new root signed.
pub(crate) fn pin(db: &Connection, root_key: &str, now_ms: i64) -> Result<(), ServerError> {
  let root = PublicKey::parse(root_key).map_err(|_| ErrorCode::Invalid)?;
  db.execute(
    "INSERT INTO cloud_link(id,root_key,accepting,linked_at) VALUES (1,?1,1,?2)
      ON CONFLICT(id) DO UPDATE SET root_key=?1,key_set=NULL,key_set_issued_at=NULL,key_set_expires_at=NULL",
    params![root.encoded(), now_ms],
  )?;
  Ok(())
}

/// Turns accepting cloud identities on or off; joined devices are not affected.
pub(crate) fn set_accepting(db: &Connection, accepting: bool) -> Result<(), ServerError> {
  let changed = db.execute("UPDATE cloud_link SET accepting=?1 WHERE id=1", [accepting])?;
  if changed == 0 {
    return Err(ErrorCode::Invalid.into());
  }
  Ok(())
}

/// Forgets the cloud: its root, its keys and the accounts' links stay usable only through codes and device tokens.
pub(crate) fn unlink(db: &Connection) -> Result<(), ServerError> {
  db.execute("DELETE FROM cloud_link WHERE id=1", [])?;
  Ok(())
}

/// What the server signs at setup for the cloud to register it: its id, name, address, key, and the fingerprint of
/// its own certificate (the SHA-256 of its DER in hex, which apps pin; `None` without one).
pub(crate) fn statement(
  keys: &ServerKeys,
  name: &str,
  address: &str,
  fingerprint: Option<String>,
  now_ms: i64,
) -> Result<String, ServerError> {
  keys.sign(
    LINK_STATEMENT_TYPE,
    &LinkStatement {
      server: keys.id,
      nonce: crate::server_keys::random::<16>()?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect(),
      name: name.to_owned(),
      address: address.to_owned(),
      public_key: keys.public_key(),
      fingerprint,
      issued_at: now_ms.div_euclid(1000),
    },
  )
}

/// Uses up a verified token when the server acts on it: its `jti` is remembered until it expires, and the token is
/// refused from then on. A join that only waits for approval leaves it unused.
pub(crate) fn consume(db: &Connection, claims: &Claims) -> Result<(), ServerError> {
  db.execute(
    "INSERT INTO used_identity_tokens(jti,expires_at) VALUES (?1,?2)",
    params![claims.jti, claims.exp + CLOCK_SKEW_SECS],
  )?;
  Ok(())
}

fn refusal(error: IdentityError) -> ServerError {
  match error {
    IdentityError::KeySetExpired => ErrorCode::KeySetStale.into(),
    _ => ErrorCode::InvalidIdentity.into(),
  }
}
