//! Discarding one reviewed change of the main checkout, never a permanent delete: what leaves the checkout is handed to
//! the caller's trash (the Recycle Bin / Trash) first, and only then does HEAD's content come back. A file is reached
//! only through held folders of the checkout that are no link or junction, by a path [`LocalPath`] allows.

use std::{io, path::Path};

use orchester_core::integration::local::{Checkout, Discard, LocalError, LocalFile, LocalPath};
use thiserror::Error;

use super::{LocalGit, LocalGitError, files::hold_directory, owned::OwnedFile};

/// The scratch folder where copies of the person's versions wait for the trash.
const COPIES: &str = "discarded";
/// How much of a copy's digest names its folder: enough to keep two versions or two files of one name apart.
const COPY_FOLDER: usize = 16;
/// What the copy of a file's staged version adds to the file's name.
pub const STAGED: &str = ".staged";

/// Moves a file to where the person can restore it, or says why the system would not.
pub type Take<'a> = &'a dyn Fn(&Path) -> io::Result<()>;

#[derive(Debug, Error)]
pub enum DiscardError {
  /// The trash refused the file, for the system's reason; the file is where it was.
  #[error("the file was not moved to the trash")]
  Trash(#[source] io::Error),
  #[error(transparent)]
  Git(#[from] LocalGitError),
}

impl From<LocalError> for DiscardError {
  fn from(error: LocalError) -> Self {
    Self::Git(error.into())
  }
}

impl LocalGit {
  /// Discards the change of `path` that `reviewed` recorded ([`Discard`]), once the file is checked to be still as
  /// reviewed. Only regular files are discarded; a link or a submodule is refused.
  pub fn discard_file(&self, reviewed: &Checkout, path: &LocalPath, take: Take<'_>) -> Result<(), DiscardError> {
    let file = reviewed
      .files
      .iter()
      .find(|file| file.path == *path)
      .ok_or(LocalError::Stale)?;
    if [&file.head, &file.staged]
      .into_iter()
      .flatten()
      .any(|entry| !matches!(entry.mode, 0o100644 | 0o100755))
    {
      return Err(LocalError::Unsupported.into());
    }
    self.ensure_file(reviewed, file)?;
    // Copies first, of every version that would otherwise be lost, before the checkout or the index changes.
    if file.keeps_staged_copy() {
      let staged = file.staged.as_ref().ok_or(LocalError::Stale)?;
      let bytes = self.blob(&staged.object)?;
      self.trash_copy(file, &format!("{}{STAGED}", name(file)), &bytes, take)?;
    }
    match (file.discard(), file.head.as_ref()) {
      (Discard::Trash, _) => {
        let expected = file.working.as_ref().ok_or(LocalError::Stale)?;
        hand_over(&self.root, &file.path, expected, take)?;
        if file.staged.is_some() {
          self.ensure_index(file)?;
          self.set_index(&file.path, None, None)?;
        }
      }
      (Discard::Unstage, _) => {
        self.ensure_index(file)?;
        self.set_index(&file.path, None, None)?;
      }
      (Discard::RestoreKeepingCopy, Some(head)) => {
        let bytes = self.file(&file.path)?.ok_or(LocalError::Stale)?;
        if file.working.as_ref() != Some(&super::digest(&bytes)) {
          return Err(LocalError::Stale.into());
        }
        self.trash_copy(file, name(file), &bytes, take)?;
        self.ensure_index(file)?;
        self.restore_head(reviewed, file, head)?;
      }
      (Discard::Restore, Some(head)) => {
        self.ensure_index(file)?;
        self.restore_head(reviewed, file, head)?;
      }
      (Discard::RestoreKeepingCopy | Discard::Restore, None) => return Err(LocalError::Stale.into()),
    }
    Ok(())
  }

  /// Refused as stale unless the index still holds the entry of `file` the review saw: checked right before the index
  /// is written, after the copies went to the trash.
  fn ensure_index(&self, file: &LocalFile) -> Result<(), LocalGitError> {
    if self.index_entries()?.get(&file.path) != file.staged.as_ref() {
      return Err(LocalError::Stale.into());
    }
    Ok(())
  }

  /// Puts `bytes`, a version of `file`, as `name` in this checkout's scratch folder and hands it to `take`.
  fn trash_copy(&self, file: &LocalFile, name: &str, bytes: &[u8], take: Take<'_>) -> Result<(), DiscardError> {
    let hash = super::digest(bytes);
    let copy = LocalPath::new(name)?;
    let key = super::digest(format!("{}\0{name}\0{}", file.path, hash.as_str()).as_bytes());
    let folder =
      hold_directory(&self.scratch, &[COPIES, &key.as_str()[..COPY_FOLDER]], true)?.ok_or(LocalGitError::Io)?;
    match super::write_regular(folder.path(), &copy, None, Some(bytes)) {
      Ok(()) => {}
      // An earlier attempt left this copy when the trash refused it: the same version of the same file.
      Err(LocalGitError::State(LocalError::Collision))
        if super::file_hash(folder.path(), &copy)?.as_ref() == Some(&hash) => {}
      Err(error) => return Err(error.into()),
    }
    // The emptied folder stays in Orchester's own scratch folder, as its other scratch files do.
    hand_over(folder.path(), &copy, &hash, take)
  }
}

/// The file's own name, which its copies carry.
fn name(file: &LocalFile) -> &str {
  file.path.as_str().rsplit('/').next().unwrap_or(file.path.as_str())
}

/// Hands the regular file `path` of `root`, which must still hold `expected`, to `take`, its folders held while it
/// moves so that no link or swapped folder stands in for them, and checks that it left.
fn hand_over(
  root: &Path,
  path: &LocalPath,
  expected: &orchester_core::blob::ContentHash,
  take: Take<'_>,
) -> Result<(), DiscardError> {
  let mut opened = OwnedFile::open(root, path, false)?.ok_or(LocalError::Stale)?;
  if opened.digest()? != *expected {
    return Err(LocalError::Stale.into());
  }
  let (folder, full) = opened.release();
  take(&full).map_err(DiscardError::Trash)?;
  folder.ensure_current()?;
  if OwnedFile::open(root, path, false)?.is_some() {
    return Err(LocalError::Reconcile.into());
  }
  Ok(())
}
