use std::{fs::OpenOptions, path::Path};

use rusqlite::{
  Connection,
  backup::{Backup, StepResult},
};

use crate::error::ServerError;

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
