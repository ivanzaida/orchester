//! Console windows of child processes. On Windows a console program started from a GUI process
//! opens its own console window unless told not to; macOS and other Unix systems never open
//! one, so there is nothing to hide there.

use std::process::Command;

#[cfg(windows)]
pub fn hide_window(process: &mut Command) {
  use std::os::windows::process::CommandExt;

  const CREATE_NO_WINDOW: u32 = 0x0800_0000;
  process.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn hide_window(_process: &mut Command) {}
