//! A folder of the test's own where a person's account folder may lie: in the applications' data of the home
//! folder, where the rules let a brought or chosen folder be (`orchester_core::account::brought_folder_problem`).
//! Windows' temporary folder already lies there (`AppData\Local\Temp`); macOS's does not (`/var/folders`), so there
//! it is a folder in `~/Library/Caches`, removed with the `TempDir`.

use std::{env, path::Path};

use tempfile::TempDir;

pub fn in_credential_place() -> TempDir {
  if cfg!(windows) {
    return tempfile::tempdir().expect("a temporary folder");
  }
  let home = env::var_os("HOME").expect("a home folder");
  tempfile::tempdir_in(Path::new(&home).join("Library").join("Caches")).expect("a folder in the home's Library")
}
