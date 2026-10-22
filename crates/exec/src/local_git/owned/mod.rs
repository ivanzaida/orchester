//! Handle-bound checkout access. Link checks describe opened objects, never a later pathname read.
//! Windows holds every ancestor against rename and each regular file against writes/deletion.
//! Unix uses no-follow directory-relative handles and never replaces an existing name during writes.
//! Conditional Unix unlink has no inode-CAS primitive here and is visibly unsupported.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
use std::{
  ffi::OsString,
  fs::File,
  io::{Read, Seek, SeekFrom, Write},
  path::Path,
  sync::Arc,
};

use orchester_core::{blob::ContentHash, integration::local::LocalPath};
#[cfg(windows)]
mod execution;
#[cfg(windows)]
mod metadata;
mod store;
#[cfg(windows)]
pub use execution::ExecutionOwnership;
#[cfg(windows)]
pub use metadata::MetadataOwnership;
pub use platform::Directory;
pub use store::StoreOwnership;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use super::{LocalError, LocalGitError, digest};

const MAX_BYTES: u64 = 16 * 1024 * 1024;

/// An opened regular object and the stable directory chain through which it was owned.
pub struct OwnedFile {
  file: File,
  parent: Arc<Directory>,
  name: OsString,
}

impl std::fmt::Debug for OwnedFile {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("OwnedFile(<private>)")
  }
}

impl OwnedFile {
  pub(crate) fn into_file(self) -> File {
    self.file
  }

  /// Lets go of the file, so another program can move it, and keeps its folder held, with the path the file has in
  /// it.
  pub(crate) fn release(self) -> (Arc<Directory>, std::path::PathBuf) {
    let path = self.parent.path().join(&self.name);
    (self.parent, path)
  }
}
impl OwnedFile {
  pub fn open(root: &Path, path: &LocalPath, writing: bool) -> Result<Option<Self>, LocalGitError> {
    let Some((parent, name)) = parent(root, path, false)? else {
      return Ok(None);
    };
    Self::at(parent, name, writing, false)
  }

  pub fn create(root: &Path, path: &LocalPath) -> Result<Self, LocalGitError> {
    let (parent, name) = parent(root, path, true)?.ok_or(LocalGitError::Io)?;
    Self::at(parent, name, true, true)?.ok_or(LocalGitError::Io)
  }

  fn at(parent: Arc<Directory>, name: OsString, writing: bool, create: bool) -> Result<Option<Self>, LocalGitError> {
    parent.ensure_current()?;
    let Some(file) = parent.open_regular(&name, writing, create)? else {
      return Ok(None);
    };
    platform::validate_regular(&file)?;
    let opened = Self { file, parent, name };
    opened.ensure_current()?;
    Ok(Some(opened))
  }

  pub fn ensure_current(&self) -> Result<(), LocalGitError> {
    self.parent.ensure_current()?;
    self.parent.same_entry(&self.name, &self.file)?;
    platform::validate_regular(&self.file)
  }

  pub fn read(&mut self) -> Result<Vec<u8>, LocalGitError> {
    self.ensure_current()?;
    self.file.seek(SeekFrom::Start(0)).map_err(|_| LocalGitError::Io)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut self.file)
      .take(MAX_BYTES + 1)
      .read_to_end(&mut bytes)
      .map_err(|_| LocalGitError::Io)?;
    if bytes.len() as u64 > MAX_BYTES {
      return Err(LocalError::Unsupported.into());
    }
    self.ensure_current()?;
    Ok(bytes)
  }

  /// The content's digest, read in chunks so a file of any size can be identified without holding it.
  pub fn digest(&mut self) -> Result<ContentHash, LocalGitError> {
    self.ensure_current()?;
    self.file.seek(SeekFrom::Start(0)).map_err(|_| LocalGitError::Io)?;
    let hash = super::digest_reader(&mut self.file).map_err(|_| LocalGitError::Io)?;
    self.ensure_current()?;
    Ok(hash)
  }

  /// The content's size and its first `limit` bytes.
  pub fn probe(&mut self, limit: u64) -> Result<(u64, Vec<u8>), LocalGitError> {
    self.ensure_current()?;
    let size = self.file.metadata().map_err(|_| LocalGitError::Io)?.len();
    self.file.seek(SeekFrom::Start(0)).map_err(|_| LocalGitError::Io)?;
    let mut head = Vec::new();
    Read::by_ref(&mut self.file)
      .take(limit)
      .read_to_end(&mut head)
      .map_err(|_| LocalGitError::Io)?;
    self.ensure_current()?;
    Ok((size, head))
  }

  pub fn executable(&self) -> Result<bool, LocalGitError> {
    self.ensure_current()?;
    platform::executable(&self.file)
  }

  pub fn set_executable(&mut self, expected: &ContentHash, value: bool) -> Result<(), LocalGitError> {
    self.compare(Some(expected))?;
    platform::set_executable(&self.file, value)?;
    self.ensure_current()
  }

  pub fn write(&mut self, expected: Option<&ContentHash>, content: &[u8]) -> Result<(), LocalGitError> {
    if content.len() as u64 > MAX_BYTES {
      return Err(LocalError::Unsupported.into());
    }
    self.compare(expected)?;
    // In-place on the held object: an entry swapped in cannot become the write target.
    self.file.seek(SeekFrom::Start(0)).map_err(|_| LocalGitError::Io)?;
    self.file.write_all(content).map_err(|_| LocalGitError::Io)?;
    self.file.set_len(content.len() as u64).map_err(|_| LocalGitError::Io)?;
    self.file.sync_all().map_err(|_| LocalGitError::Io)?;
    self.ensure_current()
  }

  pub fn remove(mut self, expected: &ContentHash) -> Result<(), LocalGitError> {
    self.compare(Some(expected))?;
    platform::remove(&self.file)
  }

  fn compare(&mut self, expected: Option<&ContentHash>) -> Result<(), LocalGitError> {
    let actual = self.read()?;
    if expected.map_or(!actual.is_empty(), |hash| digest(&actual) != *hash) {
      return Err(LocalError::Stale.into());
    }
    self.ensure_current()
  }
}

fn parent(root: &Path, path: &LocalPath, create: bool) -> Result<Option<(Arc<Directory>, OsString)>, LocalGitError> {
  let mut directory = Directory::open(root)?;
  let mut parts = path.as_str().split('/').peekable();
  while let Some(part) = parts.next() {
    if parts.peek().is_none() {
      return Ok(Some((directory, OsString::from(part))));
    }
    match directory.child(part.as_ref(), create)? {
      Some(child) => directory = child,
      None => return Ok(None),
    }
  }
  Err(LocalError::Unsupported.into())
}
