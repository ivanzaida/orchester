use std::{fs::OpenOptions, path::Path};

use rusqlite::{Connection, MAIN_DB};

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
  match source.backup(MAIN_DB, path, None) {
    Ok(()) => Ok(()),
    Err(error) => {
      // Only the file created above can be removed; a failed snapshot must never look complete.
      std::fs::remove_file(path)?;
      Err(error.into())
    }
  }
}
