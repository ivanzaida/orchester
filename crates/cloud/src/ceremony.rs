//! The offline half of the key ceremony: making the root key and signing key sets with it. Neither needs the cloud's
//! configuration, database or network, so both run on the machine that keeps the root key.

use std::{
  fs,
  io::{self, Write},
  path::Path,
};

use orchester_api::identity::{KeySetStatement, PrivateKey, RootKey};
use orchester_cloud::{
  clock::{Clock, SystemClock},
  error::CloudError,
};

/// Writes a new root key to `path` (refusing to overwrite) and prints its public half and fingerprint.
pub(crate) fn generate_root(path: &Path) -> Result<(), CloudError> {
  let private = PrivateKey::generate()?;
  let root = RootKey::new(&private)?;
  orchester_core::private_file::create(path)?.write_all(private.pkcs8())?;
  let public = root.public_key();
  let mut out = io::stdout().lock();
  writeln!(out, "Root key written to {}. Keep it offline.", path.display())?;
  writeln!(out, "ORCHESTER_CLOUD_ROOT_PUBLIC_KEY={}", public.encoded())?;
  writeln!(out, "Fingerprint team servers pin: {}", public.fingerprint())?;
  Ok(())
}

/// Signs a statement from `keys prepare` with the root key, after printing what it lists. The root refuses a statement
/// older than a day or one whose lifetimes exceed the rules (`orchester_api::identity`).
pub(crate) fn sign(statement: &Path, root: &Path, signed: &Path) -> Result<(), CloudError> {
  let statement: KeySetStatement = serde_json::from_slice(&fs::read(statement)?)
    .map_err(|_| CloudError::Configuration("the statement is not a key set statement"))?;
  let root = RootKey::new(&PrivateKey::from_pkcs8(fs::read(root)?))?;
  let now = SystemClock.now_ms() / 1000;
  let compact = root.sign_key_set(&statement, now)?;
  let mut out = io::stdout().lock();
  writeln!(out, "Issuer: {}", statement.iss)?;
  writeln!(
    out,
    "Key set valid from {} until {}",
    date(statement.issued_at),
    date(statement.expires_at)
  )?;
  for key in &statement.keys {
    writeln!(
      out,
      "Key {}: signs from {} until {}, listed until {}",
      key.kid,
      date(key.not_before),
      date(key.signs_until),
      date(key.listed_until)
    )?;
  }
  fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(signed)?
    .write_all(compact.as_bytes())?;
  writeln!(out, "Signed key set written to {}.", signed.display())?;
  Ok(())
}

/// A time of the statement, in UTC, for the person to read before the key set is published.
fn date(seconds: i64) -> String {
  chrono::DateTime::from_timestamp(seconds, 0).map_or_else(
    || format!("{seconds} (Unix seconds)"),
    |time| time.format("%Y-%m-%d %H:%M UTC").to_string(),
  )
}
