//! Runs the CLIs for connection checks. Which commands run, and why they are read-only, is
//! decided in `orchester_core::connection`; `orchester_exec` starts the processes. Everything
//! here blocks; the UI calls it off the UI thread.

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  connection::{self, AgentProvider, Connection, ConnectionCheck, ConnectionKind, LastCheck},
  time::Timestamp,
};
use orchester_exec::ProcessRunner;

/// A CLI that does not answer a status command within this time is reported as not running.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const RUNNER: ProcessRunner = ProcessRunner::new(COMMAND_TIMEOUT);

/// Checks the agent CLI at `executable` now.
pub fn check(provider: AgentProvider, executable: &Path) -> ConnectionCheck {
  connection::check(provider, executable, &RUNNER, Timestamp::now())
}

/// Checks the program of a stored connection now, whatever its kind.
pub fn recheck(connection: &Connection) -> LastCheck {
  match connection.kind() {
    ConnectionKind::Agent(provider) => LastCheck::Agent(check(*provider, connection.executable())),
    ConnectionKind::GitHub { host } => LastCheck::GitHub(connection::check_github(
      connection.executable(),
      host,
      &RUNNER,
      Timestamp::now(),
    )),
  }
}

/// Installed copies of the provider's CLI found on the `PATH`, in search order.
pub fn detect(provider: AgentProvider) -> Vec<PathBuf> {
  let path = std::env::var_os("PATH");
  let mut found: Vec<PathBuf> = Vec::new();
  for candidate in connection::executable_candidates(provider.program_name(), path.as_deref()) {
    if candidate.is_file() && !found.contains(&candidate) {
      found.push(candidate);
    }
  }
  found
}
