//! Runs the agent CLIs for connection checks. Which commands run, and why they are read-only,
//! is decided in `orchester_core::connection`; this only starts the process and collects its
//! output. Everything here blocks; the UI calls it off the UI thread.

use std::{
  io::{self, Read},
  path::{Path, PathBuf},
  process::{Command, Stdio},
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  connection::{self, AgentProvider, CommandError, CommandOutput, CommandRunner, ConnectionCheck},
  time::Timestamp,
};

/// A CLI that does not answer a status command within this time is reported as not running.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// The CLIs are console programs; without this each check would flash a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Runs programs as child processes with a timeout.
pub struct ProcessRunner;

impl CommandRunner for ProcessRunner {
  fn run(&self, program: &Path, args: &[&str]) -> Result<CommandOutput, CommandError> {
    let mut command = Command::new(program);
    command
      .args(args)
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NO_WINDOW);
    let mut child = command.spawn().map_err(|error| match error.kind() {
      io::ErrorKind::NotFound => CommandError::NotFound,
      _ => CommandError::Failed(error.to_string()),
    })?;
    // Read both pipes while waiting: a full pipe would otherwise block the program forever.
    let stdout = read_to_end(child.stdout.take());
    let stderr = read_to_end(child.stderr.take());
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    let status = loop {
      match child.try_wait() {
        Ok(Some(status)) => break status,
        Ok(None) if Instant::now() >= deadline => {
          if let Err(error) = child.kill() {
            tracing::warn!(%error, program = %program.display(), "cannot stop a CLI that did not answer");
          }
          return Err(CommandError::TimedOut);
        }
        Ok(None) => thread::sleep(POLL_INTERVAL),
        Err(error) => return Err(CommandError::Failed(error.to_string())),
      }
    };
    Ok(CommandOutput {
      success: status.success(),
      stdout: joined(stdout)?,
      stderr: joined(stderr)?,
    })
  }
}

fn read_to_end(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<io::Result<String>> {
  thread::spawn(move || {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
      pipe.read_to_end(&mut bytes)?;
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
  })
}

fn joined(reader: thread::JoinHandle<io::Result<String>>) -> Result<String, CommandError> {
  reader
    .join()
    .map_err(|_| CommandError::Failed("the output reader panicked".into()))?
    .map_err(|error| CommandError::Failed(error.to_string()))
}

/// Checks the CLI at `executable` now.
pub fn check(provider: AgentProvider, executable: &Path) -> ConnectionCheck {
  connection::check(provider, executable, &ProcessRunner, Timestamp::now())
}

/// Installed copies of the provider's CLI found on the `PATH`, in search order.
pub fn detect(provider: AgentProvider) -> Vec<PathBuf> {
  let path = std::env::var_os("PATH");
  let mut found: Vec<PathBuf> = Vec::new();
  for candidate in connection::executable_candidates(provider, path.as_deref()) {
    if candidate.is_file() && !found.contains(&candidate) {
      found.push(candidate);
    }
  }
  found
}
