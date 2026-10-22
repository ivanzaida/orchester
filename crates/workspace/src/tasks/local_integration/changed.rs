//! What a refusal as stale found: the paths a hashed snapshot of the checkout finds changed since its review, an edit
//! Git's stat cache hides among them (one that kept its file's size and modification time). Taken once after a
//! refusal, never on the page's poll, and through an owned index: the person's index is never written.

use std::{collections::BTreeSet, path::Path};

use orchester_core::{
  id::RunId,
  integration::local::{Checkout, LocalFile, LocalIntegrationStore, LocalPath},
  secret::SecretVault,
};
use orchester_exec::local_git::LocalGit;
use orchester_store::ProjectDb;

use super::{IntegrationError, view::Redaction};

/// The paths of the checkout at `root` whose content, staging or mode differ from what run `run`'s local integration
/// last reviewed, every tracked file hashed; with no review recorded, from what Git's own status shows. Refused as
/// sensitive metadata when a path holds a secret of the project's.
pub fn changed_since_review(
  root: &Path,
  run: RunId,
  vault: &dyn SecretVault,
) -> Result<Vec<LocalPath>, IntegrationError> {
  let scratch =
    orchester_exec::local_git::prepare_scratch(root, &[".orchester", "local-integration", &run.to_string()])?;
  let _store_owner = orchester_exec::local_git::owned::StoreOwnership::acquire(root)?;
  let db = ProjectDb::open(root)?;
  let main = LocalGit::new(root, &scratch)?;
  super::ensure_project_git(&main)?;
  let reviewed = match db.local_operation(run)? {
    Some(saved) => saved.reviewed().clone(),
    None => main.snapshot()?,
  };
  let changed = changed_paths(&reviewed, &main.verified_snapshot()?);
  let redact = Redaction::load(&db, vault)?;
  for path in &changed {
    redact.ensure_text_metadata(path.as_str())?;
  }
  Ok(changed)
}

/// The paths `reviewed` and `actual` list differently, in path order.
fn changed_paths(reviewed: &Checkout, actual: &Checkout) -> Vec<LocalPath> {
  fn entry<'a>(checkout: &'a Checkout, path: &LocalPath) -> Option<&'a LocalFile> {
    checkout.files.iter().find(|file| file.path == *path)
  }
  reviewed
    .files
    .iter()
    .chain(&actual.files)
    .map(|file| &file.path)
    .collect::<BTreeSet<_>>()
    .into_iter()
    .filter(|path| entry(reviewed, path) != entry(actual, path))
    .cloned()
    .collect()
}
