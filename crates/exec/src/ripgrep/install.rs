//! Installing ripgrep when a person asks for it (Project settings › Context › *Install ripgrep*):
//! `winget install --id BurntSushi.ripgrep.MSVC --exact` on Windows, `brew install ripgrep` on macOS.
//! Nothing runs without that request. Without the installer on the search path the install is
//! unavailable and says which installer is missing. A running install can be cancelled and is
//! stopped after a time limit, with the processes it started.

use std::{
  ffi::OsStr,
  io::{self, BufReader, Read},
  path::{Path, PathBuf},
  process::Child,
  sync::mpsc,
  thread,
  time::{Duration, Instant},
};

use orchester_core::command::{Command, CommandError};
use thiserror::Error;

/// How often a running installer is looked at.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

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
  /// A person cancelled the install; the installer was stopped.
  #[error("the install was cancelled")]
  Cancelled,
  /// The installer did not end within `after` and was stopped.
  #[error("the installer did not finish within {after:?}")]
  TimedOut { after: Duration },
}

/// An installer's exit code as people look it up: a small code in decimal, an HRESULT-style one
/// (winget's `0x8A150011`, negative as a signed number) in hexadecimal.
pub fn exit_code_text(code: i32) -> String {
  if (0..=0xFFFF).contains(&code) {
    code.to_string()
  } else {
    format!("0x{:08X}", code.cast_unsigned())
  }
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

  /// The command that installs ripgrep. Homebrew does not update itself first: that can take
  /// minutes and is not what a person asked for.
  pub fn command(&self) -> Command {
    let command = Command::new(&self.program, self.installer.args().iter().copied());
    match self.installer {
      Installer::Winget => command,
      Installer::Homebrew => command.with_env("HOMEBREW_NO_AUTO_UPDATE", "1"),
    }
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

  /// Starts installing ripgrep.
  pub fn start(&self) -> Result<Installing, InstallError> {
    Installing::start(&self.command())
  }
}

/// An installer that runs. Its output is read line by line as it comes; a progress line that
/// rewrites itself (ending in a carriage return) counts as a line.
pub struct Installing {
  child: Child,
  lines: mpsc::Receiver<String>,
  errors: thread::JoinHandle<io::Result<String>>,
}

impl Installing {
  /// Starts `command`, an installer.
  pub fn start(command: &Command) -> Result<Self, InstallError> {
    let mut child = crate::prepare(command)
      .map_err(InstallError::Run)?
      .spawn()
      .map_err(|error| InstallError::Run(CommandError::Failed(error.to_string())))?;
    let stdout = child.stdout.take().expect("prepare pipes standard output");
    let stderr = child.stderr.take().expect("prepare pipes standard error");
    let (sender, lines) = mpsc::channel();
    thread::spawn(move || forward_lines(stdout, &sender));
    let errors = thread::spawn(move || {
      let mut text = String::new();
      BufReader::new(stderr).read_to_string(&mut text).map(|_| text)
    });
    Ok(Self { child, lines, errors })
  }

  /// The installer's process, for a caller that ends its process tree with its own (a Job Object).
  pub const fn process(&self) -> &Child {
    &self.child
  }

  /// Waits for the installer to end, handing each line it prints to `line`. It is stopped, with
  /// what it started, once `cancelled` says so or after `timeout`.
  pub fn wait(
    mut self,
    mut line: impl FnMut(&str),
    cancelled: impl Fn() -> bool,
    timeout: Duration,
  ) -> Result<(), InstallError> {
    let failed = |error: io::Error| InstallError::Run(CommandError::Failed(error.to_string()));
    let deadline = Instant::now() + timeout;
    let mut last = String::new();
    let status = loop {
      while let Ok(text) = self.lines.try_recv() {
        line(&text);
        last = text;
      }
      if let Some(status) = self.child.try_wait().map_err(failed)? {
        break status;
      }
      let stop = if cancelled() {
        Some(InstallError::Cancelled)
      } else if Instant::now() >= deadline {
        Some(InstallError::TimedOut { after: timeout })
      } else {
        None
      };
      if let Some(stop) = stop {
        self.stop();
        return Err(stop);
      }
      thread::sleep(POLL_INTERVAL);
    };
    // The rest of the output, up to where the installer closed it.
    for text in self.lines.iter() {
      line(&text);
      last = text;
    }
    let errors = self
      .errors
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

  /// Ends the installer and reaps it: on macOS with the process group it leads, on Windows the
  /// installer itself, while the caller's Job Object ([`Installing::process`]) ends what it started.
  fn stop(&mut self) {
    if let Err(error) = crate::stop(&mut self.child) {
      tracing::warn!(%error, "cannot stop the ripgrep installer");
    }
    if let Err(error) = self.child.wait() {
      tracing::warn!(%error, "cannot confirm that the ripgrep installer ended");
    }
  }
}

/// Sends every line `output` prints to `lines`, split at line feeds and carriage returns, until
/// it closes or no one takes the lines.
fn forward_lines(mut output: impl Read, lines: &mpsc::Sender<String>) {
  let mut pending = Vec::new();
  let mut chunk = [0_u8; 4096];
  loop {
    let read = match output.read(&mut chunk) {
      Ok(0) => break,
      Ok(read) => read,
      Err(error) => {
        tracing::debug!(%error, "cannot read the ripgrep installer's output");
        break;
      }
    };
    pending.extend_from_slice(&chunk[..read]);
    while let Some(end) = pending.iter().position(|byte| matches!(byte, b'\n' | b'\r')) {
      let text = String::from_utf8_lossy(&pending[..end]).trim().to_owned();
      pending.drain(..=end);
      if !text.is_empty() && lines.send(text).is_err() {
        return;
      }
    }
  }
  let text = String::from_utf8_lossy(&pending).trim().to_owned();
  if !text.is_empty() && lines.send(text).is_err() {
    tracing::debug!("the ripgrep installer's last line came after its wait ended");
  }
}
