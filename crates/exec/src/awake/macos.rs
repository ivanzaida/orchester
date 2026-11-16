//! Holding macOS awake: `caffeinate -i`, the system's own tool, which prevents the idle sleep of the system while it
//! runs. It is started with `-w <pid of this process>`, so it ends by itself when Orchester does, however that
//! happens, and is killed when the hold is dropped. No FFI: a power assertion of this process's own would need IOKit.

use std::process::{Child, Command, Stdio};

use super::AwakeError;

/// The system's tool, at its fixed place: the search path of an app started from Finder is not the shell's.
const CAFFEINATE: &str = "/usr/bin/caffeinate";

/// The `caffeinate` child that holds the system awake.
#[derive(Debug)]
pub(super) struct Hold {
  child: Child,
}

impl Hold {
  pub(super) fn take() -> Result<Self, AwakeError> {
    Command::new(CAFFEINATE)
      .args(["-i", "-w", &std::process::id().to_string()])
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .map(|child| Self { child })
      .map_err(|error| AwakeError::Refused(error.to_string()))
  }
}

impl Drop for Hold {
  fn drop(&mut self) {
    // It may have ended by itself; either way it is waited for, so no zombie is left.
    if let Err(error) = self.child.kill() {
      tracing::debug!(%error, "caffeinate had ended already");
    }
    if let Err(error) = self.child.wait() {
      tracing::warn!(%error, "caffeinate could not be waited for");
    }
  }
}
