//! Discarding the person's selected local changes of the main checkout (*Discard selected* on a task's Local
//! integration page): each goes to the Recycle Bin / Trash or back to HEAD as [`Discard`] says, file by file, and the
//! checkout is read again. A file that could not be discarded stays as it was and says why; the others are discarded.

use orchester_core::integration::local::{Discard, LocalError, LocalPath};
use orchester_exec::local_git::{DiscardError, LocalGitError};

use super::{IntegrationError, LocalIntegration};
use crate::accounts::trash::Trash;

/// What discarding did to each selected path, in the order they were selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardReport(Vec<DiscardedFile>);

/// One selected path: what discarding it does, and why it was not discarded when it was not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardedFile {
  pub path: LocalPath,
  pub discard: Discard,
  /// `None` once it is discarded.
  pub failure: Option<DiscardFailure>,
}

/// Why a file was not discarded; it is as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscardFailure {
  /// The Recycle Bin / Trash refused it, in the system's own words, e.g. that another program has it open.
  Trash(String),
  /// It changed since the review.
  Changed,
  /// Its kind or its place can't be discarded: a link, a folder that is a link or junction, a submodule.
  Unsupported,
  /// Git refused to change the index.
  Git,
  /// It could not be read or written.
  Io,
}

impl DiscardReport {
  pub fn files(&self) -> &[DiscardedFile] {
    &self.0
  }

  /// Whether every selected file was discarded.
  pub fn complete(&self) -> bool {
    self.0.iter().all(|file| file.failure.is_none())
  }

  /// The paths that were not discarded, to try again.
  pub fn failed(&self) -> Vec<LocalPath> {
    self
      .0
      .iter()
      .filter(|file| file.failure.is_some())
      .map(|file| file.path.clone())
      .collect()
  }
}

impl LocalIntegration {
  /// Discards `paths`, each a change of the reviewed checkout, through `trash`, then reviews the checkout again: a
  /// clean one is prepared for delivery, as after a commit. Refused before anything changes unless the checkout is
  /// still as reviewed and every path is one of its changes.
  pub fn discard(&mut self, paths: &[LocalPath], trash: &dyn Trash) -> Result<DiscardReport, IntegrationError> {
    self.ensure_authorized()?;
    if paths.is_empty() {
      return Err(LocalError::Blank.into());
    }
    let current = self.main.verified_snapshot()?;
    self.operation.begin_discard(&current)?;
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
      let file = current
        .files
        .iter()
        .find(|file| file.path == *path)
        .ok_or(LocalError::Stale)?;
      if files.iter().any(|listed: &DiscardedFile| listed.path == *path) {
        return Err(LocalError::Stale.into());
      }
      files.push(DiscardedFile {
        path: path.clone(),
        discard: file.discard(),
        failure: None,
      });
    }
    let take = |path: &std::path::Path| trash.take(path);
    for file in &mut files {
      file.failure = self.main.discard_file(&current, &file.path, &take).err().map(failure);
    }
    self.operation.discard_finished(self.main.verified_snapshot()?)?;
    self.save()?;
    Ok(DiscardReport(files))
  }
}

fn failure(error: DiscardError) -> DiscardFailure {
  match error {
    DiscardError::Trash(error) => DiscardFailure::Trash(error.to_string()),
    DiscardError::Git(LocalGitError::State(LocalError::Stale)) => DiscardFailure::Changed,
    DiscardError::Git(LocalGitError::State(
      LocalError::Unsupported | LocalError::ConditionalRemovalUnsupported | LocalError::Collision,
    )) => DiscardFailure::Unsupported,
    DiscardError::Git(LocalGitError::Refused) => DiscardFailure::Git,
    DiscardError::Git(_) => DiscardFailure::Io,
  }
}
