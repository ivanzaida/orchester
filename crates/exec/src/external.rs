//! Handing files and web addresses to the platform: Explorer on Windows, `open` on macOS. Other
//! platforms have no launcher yet and get an explicit error.

use std::{io, path::Path};

/// Opens `path` with its default program. The launcher exits once it has passed the path on
/// and its exit code does not report whether that worked, so only a failure to start it is an
/// error.
pub fn open_path(path: &Path) -> io::Result<()> {
  launcher::launch(path.as_os_str())
}

/// Opens the text file `path` in the default text editor (macOS `open -t`), whatever program the file's kind
/// names. Windows names no default text editor: there it is an error, and callers open text with [`open_path`].
pub fn open_text(path: &Path) -> io::Result<()> {
  launcher::launch_text(path.as_os_str())
}

/// Opens `url` in the default browser. Only `https` addresses are handed on, so the launcher is
/// never asked to run a file or another scheme.
pub fn open_url(url: &str) -> io::Result<()> {
  if !url.starts_with("https://") {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      format!("only https addresses are opened, not {url}"),
    ));
  }
  launcher::launch(url.as_ref())
}

#[cfg(target_os = "windows")]
mod launcher {
  use std::{ffi::OsStr, io, process::Command};

  pub fn launch(target: &OsStr) -> io::Result<()> {
    Command::new("explorer.exe").arg(target).status().map(drop)
  }

  /// Windows names no default text editor; callers open text with [`launch`], for the kinds they allow.
  pub fn launch_text(target: &OsStr) -> io::Result<()> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      format!("no default text editor to open {} with on Windows", target.display()),
    ))
  }
}

#[cfg(target_os = "macos")]
mod launcher {
  use std::{ffi::OsStr, io, process::Command};

  pub fn launch(target: &OsStr) -> io::Result<()> {
    Command::new("open").arg(target).status().map(drop)
  }

  /// `open -t`: the default text editor, never the program the file's kind names.
  pub fn launch_text(target: &OsStr) -> io::Result<()> {
    Command::new("open").arg("-t").arg(target).status().map(drop)
  }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod launcher {
  use std::{ffi::OsStr, io};

  pub fn launch(target: &OsStr) -> io::Result<()> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      format!("no default program launcher for {} on this platform", target.display()),
    ))
  }

  pub fn launch_text(target: &OsStr) -> io::Result<()> {
    launch(target)
  }
}
