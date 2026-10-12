use std::{
  fs,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{blob::ContentHash, integration::local::LocalPath};

use super::{
  LocalError, LocalGitError, digest,
  owned::{Directory, OwnedFile},
};

pub fn read_regular(root: &Path, path: &LocalPath) -> Result<Option<Vec<u8>>, LocalGitError> {
  OwnedFile::open(root, path, false)?
    .map(|mut file| file.read())
    .transpose()
}

pub fn file_hash(root: &Path, path: &LocalPath) -> Result<Option<ContentHash>, LocalGitError> {
  read_regular(root, path).map(|bytes| bytes.as_deref().map(digest))
}

/// Comparison and mutation use the same opened regular object; source receipts retain interrupted writes.
pub fn write_regular(
  root: &Path,
  path: &LocalPath,
  expected: Option<&ContentHash>,
  content: Option<&[u8]>,
) -> Result<(), LocalGitError> {
  match (expected, content) {
    (Some(hash), Some(bytes)) => {
      let mut file = OwnedFile::open(root, path, true)?.ok_or(LocalError::Stale)?;
      file.write(Some(hash), bytes)
    }
    (None, Some(bytes)) => OwnedFile::create(root, path)?.write(None, bytes),
    (Some(hash), None) => OwnedFile::open(root, path, true)?
      .ok_or(LocalError::Stale)?
      .remove(hash),
    (None, None) => {
      if OwnedFile::open(root, path, false)?.is_some() {
        return Err(LocalError::Stale.into());
      }
      Ok(())
    }
  }
}

pub fn executable(root: &Path, path: &LocalPath) -> Result<bool, LocalGitError> {
  match OwnedFile::open(root, path, false)? {
    Some(file) => file.executable(),
    None => Ok(false),
  }
}

pub fn set_executable(root: &Path, path: &LocalPath, expected: &ContentHash, value: bool) -> Result<(), LocalGitError> {
  OwnedFile::open(root, path, true)?
    .ok_or(LocalError::Stale)?
    .set_executable(expected, value)
}

pub fn prepare_scratch(root: &Path, components: &[&str]) -> Result<PathBuf, LocalGitError> {
  Ok(
    hold_directory(root, components, true)?
      .ok_or(LocalGitError::Io)?
      .path()
      .to_owned(),
  )
}

pub fn owned_directory(root: &Path, components: &[&str]) -> Result<Option<PathBuf>, LocalGitError> {
  Ok(hold_directory(root, components, false)?.map(|directory| directory.path().to_owned()))
}

pub(crate) fn hold_directory(
  root: &Path,
  components: &[&str],
  create: bool,
) -> Result<Option<Arc<Directory>>, LocalGitError> {
  let mut directory = Directory::open(root)?;
  for component in components {
    if component.contains(['/', '\\']) || matches!(*component, "" | "." | "..") {
      return Err(LocalError::Unsupported.into());
    }
    match directory.child(component.as_ref(), create)? {
      Some(child) => directory = child,
      None => return Ok(None),
    }
  }
  directory.ensure_current()?;
  Ok(Some(directory))
}

pub fn owned_store(root: &Path) -> Result<(), LocalGitError> {
  let directory = hold_directory(root, &[".orchester"], true)?.ok_or(LocalGitError::Io)?;
  for name in [
    "orchester.db",
    "orchester.db-wal",
    "orchester.db-shm",
    "orchester.db-journal",
    "local-integration.lock",
  ] {
    let path = directory.path().join(name);
    match fs::symlink_metadata(&path) {
      Ok(metadata) if linked(&metadata) || !metadata.is_file() => return Err(LocalError::Unsupported.into()),
      Ok(_) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(_) => return Err(LocalGitError::Io),
    }
  }
  directory.ensure_current()
}

#[cfg(windows)]
fn linked(metadata: &fs::Metadata) -> bool {
  use std::os::windows::fs::MetadataExt;
  metadata.file_attributes() & 0x400 != 0
}
#[cfg(unix)]
fn linked(metadata: &fs::Metadata) -> bool {
  metadata.file_type().is_symlink()
}
/// Transfers one untracked regular file into a private commit copy, including its selected executable bit.
pub fn copy_untracked_file(
  source: &Path,
  destination: &Path,
  path: &LocalPath,
  executable: bool,
) -> Result<ContentHash, LocalGitError> {
  let bytes = read_regular(source, path)?.ok_or(LocalError::Stale)?;
  let hash = digest(&bytes);
  write_regular(destination, path, None, Some(&bytes))?;
  set_executable(destination, path, &hash, executable)?;
  if file_hash(destination, path)?.as_ref() != Some(&hash)
    || self::executable(destination, path)? != (cfg!(unix) && executable)
  {
    return Err(LocalError::Reconcile.into());
  }
  Ok(hash)
}
