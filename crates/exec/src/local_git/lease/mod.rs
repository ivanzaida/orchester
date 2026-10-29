//! An OS-held project lease plus stable ownership of its metadata parent chain.

#[cfg(unix)]
use std::time::{Duration, Instant};
use std::{fs::File, path::Path, sync::Arc};

use orchester_core::integration::local::LocalPath;

use super::{
  LocalError, LocalGitError,
  owned::{Directory, OwnedFile},
};

pub struct IntegrationLease {
  _file: File,
  _directory: Arc<Directory>,
}

impl IntegrationLease {
  pub fn acquire(root: &Path) -> Result<Self, LocalGitError> {
    let directory = super::files::hold_directory(root, &[".orchester"], true)?.ok_or(LocalGitError::Io)?;
    super::files::owned_store(root)?;
    let path = LocalPath::new("local-integration.lock")?;
    let opened = match OwnedFile::open(directory.path(), &path, true) {
      Ok(Some(file)) => file,
      Ok(None) => OwnedFile::create(directory.path(), &path).map_err(|_| LocalError::ForeignOperation)?,
      Err(_) => return Err(LocalError::ForeignOperation.into()),
    };
    let file = opened.into_file();
    #[cfg(unix)]
    lock(&file)?;
    Ok(Self {
      _file: file,
      _directory: directory,
    })
  }
}

/// How long the lease waits for a lock held only for a moment: a program started by `fork` and `exec` in another
/// thread holds a copy of every descriptor, this one's too, until its `exec` closes it. Another operation holds the
/// lease for far longer and is refused once the wait is over.
#[cfg(unix)]
const MOMENT: Duration = Duration::from_millis(500);

#[cfg(unix)]
fn lock(file: &File) -> Result<(), LocalGitError> {
  let deadline = Instant::now() + MOMENT;
  loop {
    match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
      Ok(()) => return Ok(()),
      Err(rustix::io::Errno::WOULDBLOCK) if Instant::now() < deadline => {
        std::thread::sleep(Duration::from_millis(5));
      }
      Err(_) => return Err(LocalError::ForeignOperation.into()),
    }
  }
}
