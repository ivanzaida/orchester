//! Which failed runner starts the desktop tries once more.

use std::{io, path::PathBuf, process::ExitStatus, time::Duration};

use orchester_ipc::{IpcError, LaunchError};

/// A runner exit with status 1.
#[cfg(windows)]
fn failed_exit() -> ExitStatus {
  std::os::windows::process::ExitStatusExt::from_raw(1)
}

/// A runner exit with status 1 (the wait status keeps the exit code in its second byte).
#[cfg(unix)]
fn failed_exit() -> ExitStatus {
  std::os::unix::process::ExitStatusExt::from_raw(1 << 8)
}

#[test]
fn launch_retries_a_runner_that_was_slow_or_exited() {
  let transient = [
    LaunchError::ConnectTimeout(Duration::from_secs(30)),
    LaunchError::RunnerExited {
      status: failed_exit(),
      stderr: "cannot start the async runtime".to_owned(),
    },
    LaunchError::Connect(io::Error::from(io::ErrorKind::ConnectionRefused)),
    LaunchError::Ipc(IpcError::ClosedDuringHandshake),
  ];
  for error in transient {
    assert!(error.is_transient(), "{error}");
  }
}

#[test]
fn launch_does_not_retry_a_missing_program_or_a_held_project() {
  let lasting = [
    LaunchError::Spawn {
      exe: PathBuf::from("orchester-runner"),
      source: io::Error::from(io::ErrorKind::NotFound),
    },
    LaunchError::ProjectLocked,
    LaunchError::ForeignPipeServer { found: 4242 },
    LaunchError::Ipc(IpcError::EndpointInUse {
      name: "orchester-runner".to_owned(),
    }),
    LaunchError::Ipc(IpcError::VersionMismatch { local: 2, remote: 1 }),
  ];
  for error in lasting {
    assert!(!error.is_transient(), "{error}");
  }
}
