//! The server's link to Orchester Cloud (`docs/architecture/server.md`, "Identity tokens and keys"): the cloud's root
//! key the server pinned, the newest key set that root signed, and whether cloud identities are accepted. Tokens are
//! verified here, without the internet; the owner can turn the cloud off, re-pin its root or unlink it at once.

pub(crate) mod identity;
pub(crate) mod jws;

use orchester_api::{CloudLink, CloudStatus, ErrorCode};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use self::identity::{Claims, IdentityError, KeySet, RootKey};
use crate::{error::ServerError, server_keys::ServerKeys};

/// The link as stored.
pub(crate) struct Link {
  root_key: RootKey,
  key_set: Option<String>,
  key_set_issued_at: Option<i64>,
  key_set_expires_at: Option<i64>,
  accepting: bool,
  linked_at: i64,
}

impl Link {
  pub fn load(db: &Connection) -> Result<Option<Self>, ServerError> {
    let row = db
      .query_row(
        "SELECT root_key,key_set,key_set_issued_at,key_set_expires_at,accepting,linked_at FROM cloud_link WHERE id=1",
        [],
        |row| {
          Ok((
            row.get::<_, String>(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
          ))
        },
      )
      .optional()?;
    row
      .map(|(root, key_set, issued, expires, accepting, linked_at)| {
        Ok(Self {
          root_key: RootKey::parse(&root).ok_or(ServerError::StoredData)?,
          key_set,
          key_set_issued_at: issued,
          key_set_expires_at: expires,
          accepting,
          linked_at,
        })
      })
      .transpose()
  }

  pub fn status(&self) -> CloudStatus {
    CloudStatus {
      root_fingerprint: self.root_key.fingerprint(),
      accepting: self.accepting,
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
    let now = now_ms.div_euclid(1000);
    if let Some(offered) = offered {
      self.offer(db, offered, now)?;
    }
    let set = self.cached(now)?;
    let claims = identity::verify_token(&set, token, &keys.id.to_string(), now).map_err(refusal)?;
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
    let Ok(set) = identity::verify_key_set(offered, &self.root_key, now) else {
      return Ok(());
    };
    if self.key_set_issued_at.is_some_and(|issued| issued >= set.issued_at) {
      return Ok(());
    }
    self.cache(db, offered, &set)
  }

  fn cache(&mut self, db: &Connection, text: &str, set: &KeySet) -> Result<(), ServerError> {
    db.execute(
      "UPDATE cloud_link SET key_set=?1,key_set_issued_at=?2,key_set_expires_at=?3 WHERE id=1",
      params![text, set.issued_at, set.expires_at],
    )?;
    self.key_set = Some(text.to_owned());
    self.key_set_issued_at = Some(set.issued_at);
    self.key_set_expires_at = Some(set.expires_at);
    Ok(())
  }

  /// The cached key set while it has not expired; `409 key_set_stale` otherwise.
  fn cached(&self, now: i64) -> Result<KeySet, ServerError> {
    let text = self.key_set.as_deref().ok_or(ErrorCode::KeySetStale)?;
    identity::verify_key_set(text, &self.root_key, now).map_err(|error| match error {
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
      linked_at: link.map(|link| link.linked_at),
    }
  }
}

/// Pins `root_key` (base64url) and caches `key_set`, which that root must have signed: the end of linking.
pub(crate) fn link(db: &Connection, root_key: &str, key_set: &str, now_ms: i64) -> Result<Link, ServerError> {
  let root = RootKey::parse(root_key).ok_or(ErrorCode::Invalid)?;
  let set = identity::verify_key_set(key_set, &root, now_ms.div_euclid(1000)).map_err(|_| ErrorCode::Invalid)?;
  db.execute(
    "INSERT INTO cloud_link(id,root_key,key_set,key_set_issued_at,key_set_expires_at,accepting,linked_at)
      VALUES (1,?1,?2,?3,?4,1,?5)
      ON CONFLICT(id) DO UPDATE SET root_key=?1,key_set=?2,key_set_issued_at=?3,key_set_expires_at=?4,accepting=1",
    params![root.encoded(), key_set, set.issued_at, set.expires_at, now_ms],
  )?;
  Link::load(db)?.ok_or(ServerError::StoredData)
}

/// Replaces the pinned root (after a root compromise). The cached key set, signed by the old root, goes with it;
/// cloud joins wait for a key set the new root signed.
pub(crate) fn pin(db: &Connection, root_key: &str, now_ms: i64) -> Result<(), ServerError> {
  let root = RootKey::parse(root_key).ok_or(ErrorCode::Invalid)?;
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

/// What the server signs at setup for the cloud to register it: its id, name, address, key and certificate.
#[derive(Serialize)]
struct LinkStatement<'a> {
  server: String,
  name: &'a str,
  address: &'a str,
  public_key: String,
  certificate: Option<String>,
  issued_at: i64,
}

pub(crate) fn statement(
  keys: &ServerKeys,
  name: &str,
  address: &str,
  certificate: Option<String>,
  now_ms: i64,
) -> String {
  keys.sign(
    "orchester-link+jws",
    &LinkStatement {
      server: keys.id.to_string(),
      name,
      address,
      public_key: keys.public_key(),
      certificate,
      issued_at: now_ms.div_euclid(1000),
    },
  )
}

/// Uses up a verified token when the server acts on it: its `jti` is remembered until it expires, and the token is
/// refused from then on. A join that only waits for approval leaves it unused.
pub(crate) fn consume(db: &Connection, claims: &Claims) -> Result<(), ServerError> {
  db.execute(
    "INSERT INTO used_identity_tokens(jti,expires_at) VALUES (?1,?2)",
    params![claims.jti, claims.exp + identity::CLOCK_SKEW_SECS],
  )?;
  Ok(())
}

fn refusal(error: IdentityError) -> ServerError {
  match error {
    IdentityError::KeySetExpired => ErrorCode::KeySetStale.into(),
    _ => ErrorCode::InvalidIdentity.into(),
  }
}
