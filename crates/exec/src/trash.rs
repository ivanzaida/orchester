//! Moves a folder to the Recycle Bin (Windows) or the Trash (macOS), where the person can still restore it; never a
//! permanent delete. Both go through the system's own shell programs, so no file system call here is undoable only
//! in part. Other platforms get an explicit error.

use std::{io, path::Path};

/// Moves `folder` to the Recycle Bin / Trash.
pub fn move_to_trash(folder: &Path) -> io::Result<()> {
  platform::move_to_trash(folder)
}

#[cfg(target_os = "windows")]
mod platform {
  use std::{io, path::Path, process::Command};

  use crate::console::hide_window;

  /// Visual Basic's file system API sends a folder to the Recycle Bin; the path reaches PowerShell in a variable,
  /// never in the script's text.
  const SCRIPT: &str = "Add-Type -AssemblyName Microsoft.VisualBasic; \
    [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($env:ORCHESTER_TRASH_FOLDER, 'OnlyErrorDialogs', \
    'SendToRecycleBin')";

  pub fn move_to_trash(folder: &Path) -> io::Result<()> {
    let mut process = Command::new("powershell.exe");
    process
      .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
      .env("ORCHESTER_TRASH_FOLDER", folder);
    hide_window(&mut process);
    let status = process.status()?;
    if status.success() {
      Ok(())
    } else {
      Err(io::Error::other(format!(
        "the folder was not moved to the Recycle Bin ({status})"
      )))
    }
  }
}

#[cfg(target_os = "macos")]
mod platform {
  use std::{io, path::Path, process::Command};

  /// Finder moves the folder to the Trash; the path is the script's argument, never part of its text.
  const SCRIPT: [&str; 3] = [
    "on run argv",
    "tell application \"Finder\" to delete (POSIX file (item 1 of argv) as alias)",
    "end run",
  ];

  pub fn move_to_trash(folder: &Path) -> io::Result<()> {
    let mut process = Command::new("osascript");
    for line in SCRIPT {
      process.args(["-e", line]);
    }
    let status = process.arg(folder).status()?;
    if status.success() {
      Ok(())
    } else {
      Err(io::Error::other(format!(
        "the folder was not moved to the Trash ({status})"
      )))
    }
  }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
  use std::{io, path::Path};

  pub fn move_to_trash(folder: &Path) -> io::Result<()> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      format!("{} can't be moved to a trash on this platform yet", folder.display()),
    ))
  }
}
