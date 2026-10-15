//! The offline half of the key ceremony: making the root key and signing key sets with it. Neither needs the cloud's
//! configuration, database or network, so both run on the machine that keeps the root key.

use std::{
  fs,
  io::{self, Write},
  path::Path,
};

use orchester_api::identity::{KeySetStatement, PrivateKey, RootKey};
use orchester_cloud::error::CloudError;

use crate::private_file;

/// Writes a new root key to `path` (refusing to overwrite) and prints its public half and fingerprint.
pub(crate) fn generate_root(path: &Path) -> Result<(), CloudError> {
  let private = PrivateKey::generate()?;
  let root = RootKey::new(&private)?;
  private_file::create(path)?.write_all(private.pkcs8())?;
  let public = root.public_key();
  let mut out = io::stdout().lock();
  writeln!(out, "Root key written to {}. Keep it offline.", path.display())?;
  writeln!(out, "ORCHESTER_CLOUD_ROOT_PUBLIC_KEY={}", public.encoded())?;
  writeln!(out, "Fingerprint team servers pin: {}", public.fingerprint())?;
  Ok(())
}

/// Signs a statement from `keys prepare` with the root key, after printing what it lists.
pub(crate) fn sign(statement: &Path, root: &Path, signed: &Path) -> Result<(), CloudError> {
  let statement: KeySetStatement = serde_json::from_slice(&fs::read(statement)?)
    .map_err(|_| CloudError::Configuration("the statement is not a key set statement"))?;
  let root = RootKey::new(&PrivateKey::from_pkcs8(fs::read(root)?))?;
  let compact = root.sign_key_set(&statement)?;
  let mut out = io::stdout().lock();
  writeln!(out, "Issuer: {}", statement.iss)?;
  writeln!(
    out,
    "Valid from {} until {} (Unix seconds)",
    statement.issued_at, statement.expires_at
  )?;
  for key in &statement.keys {
    writeln!(
      out,
      "Key {}: signs {}..{}, listed until {}",
      key.kid, key.not_before, key.signs_until, key.listed_until
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
