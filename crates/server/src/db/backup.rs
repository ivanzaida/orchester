use std::{
  fs::OpenOptions,
  path::{Path, PathBuf},
  time::Duration,
};

use rusqlite::{
  Connection, OpenFlags,
  backup::{Backup, StepResult},
};

use crate::error::ServerError;

/// Backs up an existing database without creating data, changing journal mode or running migrations.
pub async fn backup_existing(directory: PathBuf, path: PathBuf) -> Result<(), ServerError> {
  tokio::task::spawn_blocking(move || {
    let source = Connection::open_with_flags(directory.join("orchester-server.db"), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    source.busy_timeout(Duration::from_secs(5))?;
    backup(&source, &path)
  })
  .await
  .map_err(|_| ServerError::Worker)?
}

pub(crate) fn backup(source: &Connection, path: &Path) -> Result<(), ServerError> {
  let mut options = OpenOptions::new();
  options.write(true).create_new(true);
  // A snapshot holds the same sensitive data as the database. Windows inherits directory ACLs.
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  drop(options.open(path)?);
  match snapshot(source, path) {
    Ok(()) => Ok(()),
    Err(error) => {
      // Only the file created above can be removed; a failed snapshot must never look complete.
      std::fs::remove_file(path)?;
      Err(error)
    }
  }
}

fn snapshot(source: &Connection, path: &Path) -> Result<(), ServerError> {
  let mut destination = Connection::open(path)?;
  let backup = Backup::new(source, &mut destination)?;
  // One step holds the read snapshot until every page is copied, so concurrent commits cannot restart the copy.
  match backup.step(-1)? {
    StepResult::Done => Ok(()),
    _ => Err(ServerError::BackupIncomplete),
  }
}
