//! Background work at low priority, such as the warm-up command, so the person's own work and the agents' builds go
//! first. Windows starts the process in the below-normal priority class, which the programs it starts inherit; macOS
//! and other Unix systems raise the niceness of the process group the program leads ([`crate::process_group`]) as
//! soon as it started.

use std::{
  io,
  process::{Child, Command},
};

/// Starts `process`, made by [`crate::prepare`], at low priority.
#[cfg(windows)]
pub fn spawn_low(process: &mut Command) -> io::Result<Child> {
  use std::os::windows::process::CommandExt;

  const CREATE_NO_WINDOW: u32 = 0x0800_0000;
  const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
  process.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
  process.spawn()
}

/// Starts `process`, made by [`crate::prepare`], at low priority.
#[cfg(unix)]
pub fn spawn_low(process: &mut Command) -> io::Result<Child> {
  /// Niceness 10: below the default of 0, above the lowest of 19.
  const NICE: i32 = 10;
  let child = process.spawn()?;
  let group = i32::try_from(child.id())
    .ok()
    .and_then(rustix::process::Pid::from_raw)
    .ok_or_else(|| io::Error::other(format!("invalid process id {}", child.id())))?;
  if let Err(error) = rustix::process::setpriority_pgrp(Some(group), NICE) {
    // The program runs either way; only its priority is the usual one.
    tracing::warn!(%error, "cannot lower the priority of a background program");
  }
  Ok(child)
}
