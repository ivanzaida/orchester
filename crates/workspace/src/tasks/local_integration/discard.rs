//! Discarding the person's selected local changes of the main checkout (*Discard selected* on a task's Local
//! integration page): each goes to the Recycle Bin / Trash or back to HEAD as [`Discard`] says, file by file, and the
//! checkout is read again. A file that could not be discarded stays as it was and says why; the others are discarded.

use orchester_core::integration::local::{Discard, LocalError, LocalPath};
use orchester_exec::{
  local_git::{DiscardError, LocalGitError},
  trash::TrashRefusal,
};

use super::{IntegrationError, LocalIntegration};
use crate::accounts::trash::Trash;

/// What discarding did to each selected path, in the order they were selected, and whether the checkout was read
/// again afterwards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardReport {
  files: Vec<DiscardedFile>,
  reviewed_again: bool,
}

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
  /// Its drive has no Recycle Bin / Trash that keeps what it is given; nothing moved.
  NoBin,
  /// It is larger than the Recycle Bin / Trash keeps; nothing moved.
  TooLarge,
  /// The system deleted a version instead of keeping it in the Recycle Bin / Trash.
  NotKept,
  /// It went to the Recycle Bin / Trash, but the checkout still showed it afterwards.
  Uncertain,
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
    &self.files
  }

  /// Whether the checkout was read and recorded again after discarding; when not, it must be reviewed before going on.
  pub const fn reviewed_again(&self) -> bool {
    self.reviewed_again
  }

  /// Whether every selected file was discarded and the checkout read again.
  pub fn complete(&self) -> bool {
    self.reviewed_again && self.files.iter().all(|file| file.failure.is_none())
  }

  /// The paths that were not discarded, to try again.
  pub fn failed(&self) -> Vec<LocalPath> {
    self
      .files
      .iter()
      .filter(|file| file.failure.is_some())
      .map(|file| file.path.clone())
      .collect()
  }
}

impl LocalIntegration {
  /// Discards `paths`, each a change of the reviewed checkout, through `trash`, then reviews the checkout again: a
  /// clean one is prepared for delivery, as after a commit. Refused before anything changes unless the checkout is
  /// still as reviewed and every path is one of its changes. Once files moved, what moved is told even when reading
  /// the checkout again fails ([`DiscardReport::reviewed_again`]).
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
    let reviewed_again = self.review_after_discard().is_ok();
    Ok(DiscardReport { files, reviewed_again })
  }

  fn review_after_discard(&mut self) -> Result<(), IntegrationError> {
    self.operation.discard_finished(self.main.verified_snapshot()?)?;
    self.save()
  }
}

fn failure(error: DiscardError) -> DiscardFailure {
  match error {
    DiscardError::Trash(error) => match TrashRefusal::of(&error) {
      Some(TrashRefusal::NoBin) => DiscardFailure::NoBin,
      Some(TrashRefusal::TooLarge) => DiscardFailure::TooLarge,
      Some(TrashRefusal::NotKept) => DiscardFailure::NotKept,
      None => DiscardFailure::Trash(error.to_string()),
    },
    DiscardError::Git(LocalGitError::State(LocalError::Stale)) => DiscardFailure::Changed,
    DiscardError::Git(LocalGitError::State(LocalError::Reconcile)) => DiscardFailure::Uncertain,
    DiscardError::Git(LocalGitError::State(
      LocalError::Unsupported | LocalError::ConditionalRemovalUnsupported | LocalError::Collision,
    )) => DiscardFailure::Unsupported,
    DiscardError::Git(LocalGitError::Refused) => DiscardFailure::Git,
    DiscardError::Git(_) => DiscardFailure::Io,
  }
}
