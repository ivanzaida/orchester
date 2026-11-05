//! A project's runner log, opened for the person ([`super::Runners::open_log_of`]).

use std::{io, path::Path, time::SystemTime};

/// Opens today's runner log of project `project` with its default program, or the logs folder when the runner has
/// not written a log today.
pub(super) fn open_log(project: &Path) -> io::Result<()> {
  let log = orchester_ipc::runner_log_path(project, SystemTime::now());
  let target = if log.is_file() {
    log
  } else {
    orchester_ipc::runner_log_dir(project)
  };
  orchester_exec::external::open_path(&target)
}
