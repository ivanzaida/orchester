//! The helper that replaces the bundle once the app exited ([`SCRIPT`], `replace.sh`): a shell started in a process
//! group of its own, so the app's exit does not end it.

use std::{
  io,
  os::unix::process::CommandExt,
  path::Path,
  process::{Child, Command, Stdio},
};

use super::{super::handover::Handover, unpack::Unpacked};

const SCRIPT: &str = include_str!("replace.sh");

/// Starts the helper that waits for the app's pid to exit and puts `unpacked`'s bundle in the place of `installed`.
pub(super) fn start(handover: &Handover, installed: &Path, unpacked: &Unpacked) -> io::Result<Child> {
  Command::new("/bin/sh")
    .arg("-c")
    .arg(SCRIPT)
    .arg("orchester-update")
    .arg(handover.pid.to_string())
    .arg(installed)
    .arg(&unpacked.app)
    .arg(&unpacked.folder)
    .arg(&handover.result)
    .arg(&handover.log)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .process_group(0)
    .spawn()
}
