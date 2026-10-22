//! Moves a file or a folder to the Recycle Bin (Windows) or the Trash (macOS), where the person can still restore it;
//! never a permanent delete. Both go through the system's own shell programs, so no file system call here is undoable
//! only in part. A failure carries the system's own reason, e.g. that another program has the file open. Other
//! platforms get an explicit error.

use std::{io, path::Path};

/// Moves `path`, a file or a folder, to the Recycle Bin / Trash.
pub fn move_to_trash(path: &Path) -> io::Result<()> {
  platform::move_to_trash(path)
}

/// The system's reason on the program's error output, or `fallback` when it gave none.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn refusal(output: &std::process::Output, fallback: &str) -> io::Error {
  let reason = String::from_utf8_lossy(&output.stderr).trim().to_owned();
  io::Error::other(if reason.is_empty() {
    format!("{fallback} ({})", output.status)
  } else {
    reason
  })
}

#[cfg(target_os = "windows")]
mod platform {
  use std::{
    fs::OpenOptions,
    io,
    os::windows::fs::OpenOptionsExt,
    path::Path,
    process::Command,
  };

  use windows_sys::Win32::Storage::FileSystem::DELETE;

  use crate::console::hide_window;

  /// Visual Basic's file system API sends a file or a folder to the Recycle Bin; the path reaches PowerShell in a
  /// variable, never in the script's text. Its exception's message is the system's reason.
  const SCRIPT: &str = "$ErrorActionPreference = 'Stop'; try { Add-Type -AssemblyName Microsoft.VisualBasic; \
    $path = $env:ORCHESTER_TRASH_PATH; \
    if (Test-Path -LiteralPath $path -PathType Container) { \
    [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($path, 'OnlyErrorDialogs', 'SendToRecycleBin') \
    } else { \
    [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($path, 'OnlyErrorDialogs', 'SendToRecycleBin') } \
    } catch { $reason = $_.Exception; while ($reason.InnerException) { $reason = $reason.InnerException }; \
    [Console]::Error.Write($reason.Message); exit 1 }";

  pub fn move_to_trash(path: &Path) -> io::Result<()> {
    if path.is_file() {
      ensure_movable(path)?;
    }
    let mut process = Command::new("powershell.exe");
    process
      .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
      .env("ORCHESTER_TRASH_PATH", path);
    hide_window(&mut process);
    let output = process.output()?;
    if output.status.success() {
      Ok(())
    } else {
      Err(super::refusal(&output, "it was not moved to the Recycle Bin"))
    }
  }

  /// Refused with the system's reason when another program holds the file or it may not be moved (`DELETE` access,
  /// which moving needs, shared with no one): the shell would otherwise ask in a dialog of its own instead.
  fn ensure_movable(path: &Path) -> io::Result<()> {
    OpenOptions::new()
      .access_mode(DELETE)
      .share_mode(0)
      .open(path)
      .map(drop)
  }
}

#[cfg(target_os = "macos")]
mod platform {
  use std::{io, path::Path, process::Command};

  /// Finder moves the file or folder to the Trash; the path is the script's argument, never part of its text.
  const SCRIPT: [&str; 3] = [
    "on run argv",
    "tell application \"Finder\" to delete (POSIX file (item 1 of argv) as alias)",
    "end run",
  ];

  pub fn move_to_trash(path: &Path) -> io::Result<()> {
    let mut process = Command::new("osascript");
    for line in SCRIPT {
      process.args(["-e", line]);
    }
    let output = process.arg(path).output()?;
    if output.status.success() {
      Ok(())
    } else {
      Err(super::refusal(&output, "it was not moved to the Trash"))
    }
  }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
  use std::{io, path::Path};

  pub fn move_to_trash(path: &Path) -> io::Result<()> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      format!("{} can't be moved to a trash on this platform yet", path.display()),
    ))
  }
}
