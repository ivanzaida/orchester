//! The runner's own log at [`orchester_ipc::runner_log_path`], appended across runs of the same
//! day. Kept in full like every other Orchester log.

use std::{
  fs::{self, File, OpenOptions},
  io,
  path::{Path, PathBuf},
  time::SystemTime,
};

use orchester_ipc::{runner_log_dir, runner_log_path};

/// Opens today's log file. `.orchester` must already exist: the runner never initializes a
/// project, so an uninitialized one is reported as `NotFound`.
pub fn open(project_root: &Path) -> io::Result<(PathBuf, File)> {
  if !project_root.join(".orchester").is_dir() {
    return Err(io::Error::new(
      io::ErrorKind::NotFound,
      "the project has no .orchester directory",
    ));
  }
  fs::create_dir_all(runner_log_dir(project_root))?;
  let path = runner_log_path(project_root, SystemTime::now());
  let file = OpenOptions::new().create(true).append(true).open(&path)?;
  Ok((path, file))
}
