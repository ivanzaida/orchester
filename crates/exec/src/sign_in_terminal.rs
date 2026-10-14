//! Opens an agent CLI's own login in a terminal window the person sees (`docs/design/agent-accounts.md`, "Flows"):
//! on Windows a new console window running the CLI, on macOS a Terminal window running a small script. Orchester
//! never sees what is typed there; it only learns when the login's command has ended, to check the account again.

use std::{io, path::Path};

/// The CLI's login, as the terminal runs it.
#[derive(Clone, Copy, Debug)]
pub struct Login<'a> {
  pub program: &'a Path,
  pub args: &'a [&'a str],
  /// The account's folder variable and folder; `None` for the CLI's own folder.
  pub variable: Option<(&'a str, &'a Path)>,
  /// The environment the CLI runs with on Windows, the one agents get; Terminal on macOS starts the person's own.
  pub environment: &'a [(String, String)],
  /// A folder for the script Terminal runs on macOS.
  pub scratch: &'a Path,
}

/// A terminal window running a login.
#[derive(Debug)]
pub struct SignInTerminal(platform::Running);

impl SignInTerminal {
  /// Opens the window.
  pub fn open(login: &Login<'_>) -> io::Result<Self> {
    platform::open(login).map(Self)
  }

  /// Blocks until the login's command has ended (the person closed the window or the CLI finished).
  pub fn wait(self) -> io::Result<()> {
    platform::wait(self.0)
  }
}

#[cfg(target_os = "windows")]
mod platform {
  use std::{
    io,
    os::windows::process::CommandExt,
    process::{Child, Command},
  };

  use super::Login;

  /// The console program gets a console window of its own instead of none.
  const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

  #[derive(Debug)]
  pub struct Running(Child);

  pub fn open(login: &Login<'_>) -> io::Result<Running> {
    let mut process = Command::new(login.program);
    process
      .args(login.args)
      .env_clear()
      .envs(login.environment.iter().map(|(name, value)| (name, value)))
      .creation_flags(CREATE_NEW_CONSOLE);
    if let Some((name, folder)) = login.variable {
      process.env(name, folder);
    }
    process.spawn().map(Running)
  }

  pub fn wait(mut running: Running) -> io::Result<()> {
    running.0.wait().map(drop)
  }
}

#[cfg(target_os = "macos")]
mod platform {
  use std::{
    fs, io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
  };

  use super::Login;

  /// How often the end of the script is looked for, and how long at most: a window closed before the login ended
  /// never marks it, and *Check again* still works then.
  const POLL: Duration = Duration::from_millis(500);
  const LONGEST: Duration = Duration::from_secs(60 * 60);

  #[derive(Debug)]
  pub struct Running {
    script: PathBuf,
    done: PathBuf,
  }

  pub fn open(login: &Login<'_>) -> io::Result<Running> {
    fs::create_dir_all(login.scratch)?;
    let script = tempfile::Builder::new()
      .prefix("sign-in-")
      .suffix(".command")
      .tempfile_in(login.scratch)?
      .into_temp_path()
      .keep()
      .map_err(|error| error.error)?;
    let done = script.with_extension("done");
    fs::write(&script, text(login, &done)?)?;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700))?;
    let status = Command::new("open").args(["-a", "Terminal"]).arg(&script).status()?;
    if !status.success() {
      return Err(io::Error::other(format!("Terminal did not open ({status})")));
    }
    Ok(Running { script, done })
  }

  fn text(login: &Login<'_>, done: &Path) -> io::Result<String> {
    let mut text = String::from("#!/bin/sh\n");
    if let Some((name, folder)) = login.variable {
      text.push_str(&format!("export {name}={}\n", quoted(folder)?));
    }
    let mut command = vec![quoted(login.program)?];
    command.extend(login.args.iter().map(|arg| format!("'{arg}'")));
    text.push_str(&command.join(" "));
    text.push_str(&format!("\n: > {}\n", quoted(done)?));
    Ok(text)
  }

  /// `path` in single quotes for `sh`, a `'` in it closed, escaped and reopened.
  fn quoted(path: &Path) -> io::Result<String> {
    let text = path
      .to_str()
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path is not valid Unicode"))?;
    Ok(format!("'{}'", text.replace('\'', r"'\''")))
  }

  pub fn wait(running: Running) -> io::Result<()> {
    let started = Instant::now();
    while !running.done.exists() && started.elapsed() < LONGEST {
      thread::sleep(POLL);
    }
    // Both are this module's own scratch files; one already gone is what removing it was for.
    for file in [running.script, running.done] {
      if let Err(error) = fs::remove_file(file)
        && error.kind() != io::ErrorKind::NotFound
      {
        return Err(error);
      }
    }
    Ok(())
  }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
  use std::io;

  use super::Login;

  #[derive(Debug)]
  pub struct Running;

  pub fn open(_login: &Login<'_>) -> io::Result<Running> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "opening a sign-in terminal is not supported on this platform yet",
    ))
  }

  pub fn wait(_running: Running) -> io::Result<()> {
    Ok(())
  }
}
