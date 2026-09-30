//! Installing ripgrep when a person asks for it (Project settings › Code index › *Install ripgrep*):
//! `winget install --id BurntSushi.ripgrep.MSVC --exact` on Windows, `brew install ripgrep` on macOS.
//! Nothing runs without that request. Without the installer on the search path the install is
//! unavailable and says which installer is missing.

use std::{
  ffi::OsStr,
  path::{Path, PathBuf},
};

use orchester_core::command::{Command, CommandError, CommandRunner};
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

  /// Runs the install with `runner` and waits for it.
  pub fn install(&self, runner: &impl CommandRunner) -> Result<(), InstallError> {
    let output = runner.run(&self.command()).map_err(InstallError::Run)?;
    if output.success() {
      return Ok(());
    }
    let message = [&output.stderr, &output.stdout]
      .into_iter()
      .find_map(|text| text.lines().rev().map(str::trim).find(|line| !line.is_empty()))
      .unwrap_or_default()
      .to_owned();
    Err(InstallError::Failed {
      code: output.code,
      message,
    })
  }
}
