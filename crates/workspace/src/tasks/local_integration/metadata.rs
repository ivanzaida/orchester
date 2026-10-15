use std::path::Path;

use orchester_core::secret::SecretVault;
use orchester_store::ProjectDb;

use super::{IntegrationError, view::Redaction};

/// The local-integration shell uses the same known-secret policy as its checkout projection.
pub fn metadata_allowed(root: &Path, values: &[String], vault: &dyn SecretVault) -> Result<bool, IntegrationError> {
  let _owner = orchester_exec::local_git::owned::StoreOwnership::acquire(root)?;
  let db = ProjectDb::open(root)?;
  let redact = Redaction::load(&db, vault)?;
  Ok(values.iter().all(|value| redact.ensure_text_metadata(value).is_ok()))
}
