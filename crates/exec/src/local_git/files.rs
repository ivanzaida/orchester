use std::{fs, path::Path};

use orchester_core::{blob::ContentHash, integration::local::LocalPath};

use super::{LocalError, LocalGitError, digest};

const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// Reject links at every component, including Windows directory junctions. Never follow them for checkout writes.
fn checked_path(root: &Path, path: &LocalPath) -> Result<std::path::PathBuf, LocalGitError> {
  let mut resolved = root.to_path_buf();
  for component in path.as_str().split('/') {
    resolved.push(component);
    match fs::symlink_metadata(&resolved) {
      Ok(metadata) if linked(&metadata) => return Err(LocalError::Unsupported.into()),
      Ok(_) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(_) => return Err(LocalGitError::Io),
    }
  }
  Ok(resolved)
}

#[cfg(windows)]
fn linked(metadata: &fs::Metadata) -> bool {
  use std::os::windows::fs::MetadataExt;
  metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn linked(metadata: &fs::Metadata) -> bool {
  metadata.file_type().is_symlink()
}

pub fn read_regular(root: &Path, path: &LocalPath) -> Result<Option<Vec<u8>>, LocalGitError> {
  let path = checked_path(root, path)?;
  let metadata = match fs::symlink_metadata(&path) {
    Ok(metadata) => metadata,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(_) => return Err(LocalGitError::Io),
  };
  if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
    return Err(LocalError::Unsupported.into());
  }
  fs::read(path).map(Some).map_err(|_| LocalGitError::Io)
}

pub fn file_hash(root: &Path, path: &LocalPath) -> Result<Option<ContentHash>, LocalGitError> {
  read_regular(root, path).map(|bytes| bytes.as_deref().map(digest))
}

/// The expected file fingerprint is checked immediately before writing; no unrelated file may be overwritten.
pub fn write_regular(
  root: &Path,
  path: &LocalPath,
  expected: Option<&ContentHash>,
  content: Option<&[u8]>,
) -> Result<(), LocalGitError> {
  if file_hash(root, path)?.as_ref() != expected {
    return Err(LocalError::Stale.into());
  }
  let resolved = checked_path(root, path)?;
  match content {
    Some(bytes) => {
      if let Some(parent) = resolved.parent() {
        fs::create_dir_all(parent).map_err(|_| LocalGitError::Io)?;
      }
      use std::io::Write;
      let scratch = scratch_directory(root)?;
      let mut temporary = tempfile::NamedTempFile::new_in(scratch).map_err(|_| LocalGitError::Io)?;
      if expected.is_some() {
        let permissions = fs::metadata(&resolved).map_err(|_| LocalGitError::Io)?.permissions();
        temporary
          .as_file()
          .set_permissions(permissions)
          .map_err(|_| LocalGitError::Io)?;
      }
      temporary.write_all(bytes).map_err(|_| LocalGitError::Io)?;
      temporary.as_file().sync_all().map_err(|_| LocalGitError::Io)?;
      // Recheck after writing scratch; interruption cannot leave a partially written result file.
      if file_hash(root, path)?.as_ref() != expected {
        return Err(LocalError::Stale.into());
      }
      if expected.is_none() {
        temporary
          .persist_noclobber(resolved)
          .map_err(|_| LocalError::Collision)?;
      } else {
        temporary.persist(resolved).map_err(|_| LocalGitError::Io)?;
      }
    }
    None if expected.is_some() => fs::remove_file(resolved).map_err(|_| LocalGitError::Io)?,
    None => {}
  }
  Ok(())
}

pub fn executable(root: &Path, path: &LocalPath) -> Result<bool, LocalGitError> {
  let path = checked_path(root, path)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    match fs::metadata(path) {
      Ok(metadata) => Ok(metadata.permissions().mode() & 0o111 != 0),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
      Err(_) => Err(LocalGitError::Io),
    }
  }
  #[cfg(not(unix))]
  {
    let _path = path;
    Ok(false)
  }
}
fn scratch_directory(root: &Path) -> Result<std::path::PathBuf, LocalGitError> {
  prepare_scratch(root, &[".orchester", "local-integration"])
}

pub fn prepare_scratch(root: &Path, components: &[&str]) -> Result<std::path::PathBuf, LocalGitError> {
  let mut scratch = root.canonicalize().map_err(|_| LocalGitError::Io)?;
  for component in components {
    if component.contains(['/', '\\']) || matches!(*component, "" | "." | "..") {
      return Err(LocalError::Unsupported.into());
    }
    scratch.push(component);
    match fs::symlink_metadata(&scratch) {
      Ok(metadata) if linked(&metadata) || !metadata.is_dir() => return Err(LocalError::Unsupported.into()),
      Ok(_) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
        fs::create_dir(&scratch).map_err(|_| LocalGitError::Io)?
      }
      Err(_) => return Err(LocalGitError::Io),
    }
  }
  Ok(scratch)
}

pub fn set_executable(root: &Path, path: &LocalPath, expected: &ContentHash, value: bool) -> Result<(), LocalGitError> {
  if file_hash(root, path)?.as_ref() != Some(expected) {
    return Err(LocalError::Stale.into());
  }
  let path = checked_path(root, path)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path.clone()).map_err(|_| LocalGitError::Io)?.permissions();
    let original = permissions.mode();
    permissions.set_mode(if value { original | 0o111 } else { original & !0o111 });
    fs::set_permissions(path, permissions).map_err(|_| LocalGitError::Io)?;
  }
  #[cfg(not(unix))]
  {
    let _supported_attributes = (path, value);
  }
  Ok(())
}
pub fn owned_directory(root: &Path, components: &[&str]) -> Result<Option<std::path::PathBuf>, LocalGitError> {
  let root = root.canonicalize().map_err(|_| LocalGitError::Io)?;
  let mut directory = root.clone();
  for component in components {
    if component.contains(['/', '\\']) || matches!(*component, "" | "." | "..") {
      return Err(LocalError::Unsupported.into());
    }
    directory.push(component);
    match fs::symlink_metadata(&directory) {
      Ok(metadata) if linked(&metadata) || !metadata.is_dir() => return Err(LocalError::Unsupported.into()),
      Ok(_) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
      Err(_) => return Err(LocalGitError::Io),
    }
  }
  let canonical = directory.canonicalize().map_err(|_| LocalGitError::Io)?;
  if !canonical.starts_with(root) {
    return Err(LocalError::Unsupported.into());
  }
  Ok(Some(directory))
}
pub fn owned_store(root: &Path) -> Result<(), LocalGitError> {
  let directory = prepare_scratch(root, &[".orchester"])?;
  for name in [
    "orchester.db",
    "orchester.db-wal",
    "orchester.db-shm",
    "orchester.db-journal",
    "local-integration.lock",
  ] {
    match fs::symlink_metadata(directory.join(name)) {
      Ok(metadata) if linked(&metadata) || !metadata.is_file() => return Err(LocalError::Unsupported.into()),
      Ok(_) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(_) => return Err(LocalGitError::Io),
    }
  }
  Ok(())
}
