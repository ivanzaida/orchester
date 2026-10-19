//! Handing files and web addresses to the platform: Explorer on Windows, `open` on macOS. Other
//! platforms have no launcher yet and get an explicit error. An editor the person has is started directly
//! ([`start_editor`]).

use std::{
  ffi::{OsStr, OsString},
  io,
  path::Path,
};

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

/// Starts the editor `program` with `args` (a folder and files to open) and returns once it has started: the editor
/// runs on its own, and a thread only waits to collect its exit. A console window of a script launcher (VS Code's
/// `code.cmd` on Windows) is not shown; arguments `cmd.exe` would read again are refused there by the standard
/// library.
pub fn start_editor(program: &Path, args: &[&Path]) -> io::Result<()> {
  let mut process = std::process::Command::new(program);
  process
    .args(args)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null());
  crate::console::hide_window(&mut process);
  let mut child = process.spawn()?;
  let waiting = std::thread::Builder::new()
    .name("editor-exit".to_owned())
    .spawn(move || {
      if let Err(error) = child.wait() {
        tracing::warn!(%error, "cannot collect the editor's exit");
      }
    });
  // The editor started either way; only its exit goes uncollected.
  if let Err(error) = waiting {
    tracing::warn!(%error, "cannot wait for the editor's exit");
  }
  Ok(())
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

/// Opens `url`, a page of a team server the app talks to over plain HTTP (one on this computer, or one the person
/// allowed plain HTTP for), in the default browser. Only `http` addresses are handed on; the caller has checked the
/// address against that server, and passes nothing else here.
pub fn open_plain_http_url(url: &str) -> io::Result<()> {
  if !url.starts_with("http://") {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      format!("only http addresses are opened here, not {url}"),
    ));
  }
  launcher::launch(url.as_ref())
}

/// `target` as Explorer's command line takes it: in double quotes, always. Rust's own quoting leaves an argument
/// without spaces bare, and Explorer splits a bare argument at commas (`a.hta,z\notes.txt` would open `a.hta`),
/// so the launcher passes this as it is. A Windows path cannot hold `"`; a target that does is refused, since it
/// would end the quotes.
pub fn explorer_argument(target: &OsStr) -> io::Result<OsString> {
  if target.as_encoded_bytes().contains(&b'"') {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      format!("{} holds a double quote", target.display()),
    ));
  }
  let mut quoted = OsString::with_capacity(target.len() + 2);
  quoted.push("\"");
  quoted.push(target);
  quoted.push("\"");
  Ok(quoted)
}

#[cfg(target_os = "windows")]
mod launcher {
  use std::{ffi::OsStr, io, os::windows::process::CommandExt, process::Command};

  pub fn launch(target: &OsStr) -> io::Result<()> {
    Command::new("explorer.exe")
      .raw_arg(super::explorer_argument(target)?)
      .status()
      .map(drop)
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
