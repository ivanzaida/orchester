//! Installing ripgrep when a person asks for it (Project settings › Code index › *Install ripgrep*):
//! `winget install --id BurntSushi.ripgrep.MSVC --exact` on Windows, `brew install ripgrep` on macOS.
//! Nothing runs without that request. Without the installer on the search path the install is
//! unavailable and says which installer is missing.

use std::{
  ffi::OsStr,
  io::{self, BufRead, BufReader, Read},
  path::{Path, PathBuf},
  process::Stdio,
  thread,
};

use orchester_core::command::{Command, CommandError};
use thiserror::Error;

/// The package manager that installs ripgrep on this platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Installer {
  Winget,
  Homebrew,
}

impl Installer {
  /// The installer of this platform; `None` where Orchester has none.
  pub const fn of_platform() -> Option<Self> {
    if cfg!(windows) {
      Some(Self::Winget)
    } else if cfg!(target_os = "macos") {
      Some(Self::Homebrew)
    } else {
      None
    }
  }

  const fn program(self) -> &'static str {
    match self {
      Self::Winget => "winget.exe",
      Self::Homebrew => "brew",
    }
  }

  const fn args(self) -> &'static [&'static str] {
    match self {
      Self::Winget => &["install", "--id", "BurntSushi.ripgrep.MSVC", "--exact"],
      Self::Homebrew => &["install", "ripgrep"],
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum InstallUnavailable {
  #[error("Orchester cannot install ripgrep on this platform")]
  Unsupported,
  /// The installer's program is not on the search path.
  #[error("{installer:?} was not found")]
  InstallerMissing { installer: Installer },
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum InstallError {
  #[error("cannot run the installer: {0}")]
  Run(CommandError),
  /// The installer ran and failed; `message` is the last line it printed.
  #[error("the installer exited with {code:?}: {message}")]
  Failed { code: Option<i32>, message: String },
}

/// How ripgrep is installed here: the installer and its program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallPlan {
  pub installer: Installer,
  program: PathBuf,
}

impl InstallPlan {
  /// The plan for this platform, with its installer found on `search_path`.
  pub fn find(search_path: Option<&OsStr>) -> Result<Self, InstallUnavailable> {
    let installer = Installer::of_platform().ok_or(InstallUnavailable::Unsupported)?;
    let program = search_path
      .and_then(|path| {
        std::env::split_paths(path)
          .map(|folder| folder.join(installer.program()))
          .find(|candidate| candidate.is_file())
      })
      .ok_or(InstallUnavailable::InstallerMissing { installer })?;
    Ok(Self { installer, program })
  }

  /// The command that installs ripgrep.
  pub fn command(&self) -> Command {
    Command::new(&self.program, self.installer.args().iter().copied())
  }

  pub fn program(&self) -> &Path {
    &self.program
  }

  /// The command line a person reads: `winget install --id BurntSushi.ripgrep.MSVC --exact`.
  pub fn command_line(&self) -> String {
    let name = match self.installer {
      Installer::Winget => "winget",
      Installer::Homebrew => "brew",
    };
    std::iter::once(name)
      .chain(self.installer.args().iter().copied())
      .collect::<Vec<_>>()
      .join(" ")
  }

  /// Runs the install and waits for it, handing each line the installer prints (a progress line
  /// that rewrites itself counts as a line) to `line`.
  pub fn install(&self, mut line: impl FnMut(&str)) -> Result<(), InstallError> {
    let mut process = crate::prepare(&self.command()).map_err(InstallError::Run)?;
    let mut child = process
      .stdin(Stdio::null())
      .spawn()
      .map_err(|error| InstallError::Run(CommandError::Failed(error.to_string())))?;
    let failed = |error: io::Error| InstallError::Run(CommandError::Failed(error.to_string()));
    let stderr = child.stderr.take().expect("prepare pipes standard error");
    let errors = thread::spawn(move || {
      let mut text = String::new();
      BufReader::new(stderr).read_to_string(&mut text).map(|_| text)
    });
    let mut last = String::new();
    if let Some(stdout) = child.stdout.take() {
      let mut reader = BufReader::new(stdout);
      let mut buffer = Vec::new();
      // A progress line ends with a carriage return, a finished line with a line feed.
      while reader.read_until(b'\n', &mut buffer).map_err(failed)? > 0 {
        for part in String::from_utf8_lossy(&buffer).split('\r') {
          let part = part.trim();
          if !part.is_empty() {
            part.clone_into(&mut last);
            line(part);
          }
        }
        buffer.clear();
      }
    }
    let status = child.wait().map_err(failed)?;
    let errors = errors
      .join()
      .expect("the error reader does not panic")
      .map_err(failed)?;
    if status.success() {
      return Ok(());
    }
    let message = errors
      .lines()
      .rev()
      .map(str::trim)
      .find(|line| !line.is_empty())
      .map_or(last, str::to_owned);
    Err(InstallError::Failed {
      code: status.code(),
      message,
    })
  }
}
