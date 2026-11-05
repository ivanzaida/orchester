//! An OS-held project lease plus stable ownership of its metadata parent chain.

#[cfg(unix)]
use std::time::{Duration, Instant};
use std::{fs::File, path::Path, sync::Arc};

use orchester_core::integration::local::LocalPath;

use super::{
  LocalError, LocalGitError,
  owned::{Directory, OwnedFile},
};

/// The lease's file in the project's `.orchester` folder.
const LOCK: &str = "local-integration.lock";

pub struct IntegrationLease {
  _file: File,
  _directory: Arc<Directory>,
}

impl IntegrationLease {
  /// Takes the project's integration lease. [`LocalError::ForeignOperation`] says another operation holds it; any
  /// other error, with its cause, that it could not be taken.
  pub fn acquire(root: &Path) -> Result<Self, LocalGitError> {
    let directory = super::files::held_directory(root, &[".orchester"])?;
    super::files::owned_store(root)?;
    let path = LocalPath::new(LOCK)?;
    let opened = match OwnedFile::open(directory.path(), &path, true) {
      Ok(Some(file)) => file,
      Ok(None) => match OwnedFile::create(directory.path(), &path) {
        Ok(file) => file,
        // Another operation created it since it was found missing.
        Err(LocalGitError::State(LocalError::Collision)) => return Err(LocalError::ForeignOperation.into()),
        Err(error) => return Err(error),
      },
      #[cfg(windows)]
      Err(error) if held(&error) => return Err(LocalError::ForeignOperation.into()),
      Err(error) => return Err(error),
    };
    let file = opened.into_file();
    #[cfg(unix)]
    lock(&file, &directory.path().join(LOCK))?;
    Ok(Self {
      _file: file,
      _directory: directory,
    })
  }
}

/// Whether opening the lease's file failed because another operation has it open: the lease is the open file itself,
/// shared for reading only, so a second opening for writing is a sharing or lock violation.
#[cfg(windows)]
fn held(error: &LocalGitError) -> bool {
  const ERROR_SHARING_VIOLATION: i32 = 32;
  const ERROR_LOCK_VIOLATION: i32 = 33;
  matches!(error, LocalGitError::Io(cause) if matches!(cause.code(), Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)))
}

/// How long the lease waits for a lock held only for a moment: a program started by `fork` and `exec` in another
/// thread holds a copy of every descriptor, this one's too, until its `exec` closes it. Another operation holds the
/// lease for far longer and is refused once the wait is over.
#[cfg(unix)]
const MOMENT: Duration = Duration::from_millis(500);

/// Locks `file`, the lease's file at `path`.
#[cfg(unix)]
fn lock(file: &File, path: &Path) -> Result<(), LocalGitError> {
  let deadline = Instant::now() + MOMENT;
  loop {
    match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
      Ok(()) => return Ok(()),
      Err(rustix::io::Errno::WOULDBLOCK) if Instant::now() < deadline => {
        std::thread::sleep(Duration::from_millis(5));
      }
      Err(rustix::io::Errno::WOULDBLOCK) => return Err(LocalError::ForeignOperation.into()),
      Err(error) => return Err(LocalGitError::io_at(path)(std::io::Error::from(error))),
    }
  }
}
