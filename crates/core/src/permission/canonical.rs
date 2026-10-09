//! Paths with links resolved as far as they exist, shared by the writable folders and the native agent's
//! file scope (`docs/architecture/native-agent.md` §5.1).

use std::{
  fs, io,
  path::{Component, Path, PathBuf, Prefix},
};

/// A path with links resolved as far as it exists.
pub(crate) struct Existing {
  pub(crate) path: PathBuf,
  /// Part of the path does not exist yet; those components are kept as written.
  pub(crate) missing: bool,
}

/// Why [`existing`] could not resolve a path: `path` is the ancestor that failed.
pub(crate) struct ExistingError {
  pub(crate) path: PathBuf,
  pub(crate) error: io::Error,
}

/// `path` with links resolved as far as it exists: its longest existing ancestor canonicalised, the
/// components after it appended unchanged. A path with a link that leads to another machine is refused before it is
/// resolved ([`super::link_targets`]).
pub(crate) fn existing(path: &Path) -> Result<Existing, ExistingError> {
  if let Err(error) = super::link_targets::stay_local(path) {
    return Err(ExistingError {
      path: path.to_path_buf(),
      error,
    });
  }
  let mut missing = Vec::new();
  let mut current = path;
  loop {
    match fs::canonicalize(current) {
      Ok(found) => {
        let mut resolved = plain(found);
        let is_missing = !missing.is_empty();
        resolved.extend(missing.iter().rev());
        return Ok(Existing {
          path: resolved,
          missing: is_missing,
        });
      }
      Err(error) if error.kind() == io::ErrorKind::NotFound => {
        let (Some(parent), Some(name)) = (current.parent(), current.file_name()) else {
          return Err(ExistingError {
            path: path.to_path_buf(),
            error,
          });
        };
        missing.push(name.to_owned());
        current = parent;
      }
      Err(error) => {
        return Err(ExistingError {
          path: current.to_path_buf(),
          error,
        });
      }
    }
  }
}

/// `path` without `.` and `..` components, without reading the file system.
pub(crate) fn normalize(path: &Path) -> PathBuf {
  let mut normalized = PathBuf::new();
  for component in path.components() {
    match component {
      Component::CurDir => {}
      Component::ParentDir => {
        normalized.pop();
      }
      other => normalized.push(other.as_os_str()),
    }
  }
  normalized
}

/// A canonical Windows path (`\\?\C:\…`) in the usual form (`C:\…`) that programs given it as an
/// argument understand; other paths unchanged.
pub(crate) fn plain(path: PathBuf) -> PathBuf {
  let mut components = path.components();
  match components.next() {
    Some(Component::Prefix(prefix)) => match prefix.kind() {
      Prefix::VerbatimDisk(letter) => PathBuf::from(format!("{}:", char::from(letter))).join(components.as_path()),
      _ => path,
    },
    _ => path,
  }
}
