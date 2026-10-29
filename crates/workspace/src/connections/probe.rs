//! Runs the CLIs for connection checks. Which commands run, and why they are read-only, is
//! decided in `orchester_core::connection`; `orchester_exec` starts the processes. Everything
//! here blocks; the UI calls it off the UI thread.

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  connection::{
    self, AgentProvider, Connection, ConnectionAccount, ConnectionCheck, ConnectionKind, GitHubCheck, GitHubHost,
    LastCheck,
    invocation::{account_environment, agent_environment},
  },
  repository::{self, DetectRequest, Detection},
  time::Timestamp,
};
use orchester_exec::{
  ProcessRunner,
  search_path::{ProgramLookup, SearchPath},
};

/// A CLI that does not answer a status command within this time is reported as not running.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const RUNNER: ProcessRunner = ProcessRunner::new(COMMAND_TIMEOUT);

/// Checks the agent CLI at `executable` on its Default account now; `None` for a native provider, which has no CLI.
pub fn check(provider: AgentProvider, executable: &Path) -> Option<ConnectionCheck> {
  check_on(provider, executable, &ConnectionAccount::Default)
}

/// Checks the agent CLI at `executable` on `account` now, with the account's folder variable set as agents get it;
/// `None` for a native provider, which has no CLI.
pub fn check_on(provider: AgentProvider, executable: &Path, account: &ConnectionAccount) -> Option<ConnectionCheck> {
  let environment = match account_environment(agent_environment(std::env::vars_os()), account, provider) {
    Ok(environment) => environment,
    // A folder whose path can't reach the CLI is one it can't run with.
    Err(error) => {
      tracing::warn!(%error, "an account's folder cannot be passed to its CLI");
      return Some(ConnectionCheck {
        at: Timestamp::now(),
        result: connection::CheckResult::CouldNotRun,
      });
    }
  };
  connection::check(provider, executable, &RUNNER, &environment, Timestamp::now())
}

/// Checks the GitHub CLI at `executable` for `host` now.
pub fn check_github(host: &GitHubHost, executable: &Path) -> GitHubCheck {
  connection::check_github(executable, host, &RUNNER, Timestamp::now())
}

/// Detects the GitHub repository of `request.repo_root` now.
pub fn detect_repository(request: &DetectRequest<'_>) -> Detection {
  repository::detect_repository(&RUNNER, request, Timestamp::now())
}

/// Checks the program of a stored connection now, whatever its kind; `None` for a native agent
/// connection, which runs no program: its project's runner tests it (`super::native::test_in`).
pub fn recheck(connection: &Connection) -> Option<LastCheck> {
  let executable = connection.executable()?;
  Some(match connection.kind() {
    ConnectionKind::Agent(provider) => LastCheck::Agent(check_on(*provider, executable, connection.account())?),
    ConnectionKind::GitHub { host } => LastCheck::GitHub(check_github(host, executable)),
  })
}

/// Installed copies of the agent CLI of `provider` on the search path, newest version first; copies
/// whose version cannot be read come last. Each copy is asked for its version only. A native
/// provider has no program to find.
pub fn detect_agent(provider: AgentProvider) -> Vec<PathBuf> {
  match find_agent(provider) {
    Some(ProgramLookup::Found(found)) => found,
    Some(ProgramLookup::NotFound { .. }) | None => Vec::new(),
  }
}

/// The installed copies of the agent CLI of `provider`, ranked as [`detect_agent`] ranks them, or the
/// folders searched when there is none; `None` for a native provider, which has no program to find.
pub fn find_agent(provider: AgentProvider) -> Option<ProgramLookup> {
  let program = provider.program_name()?;
  Some(match SearchPath::current().lookup(program) {
    ProgramLookup::Found(found) => ProgramLookup::Found(
      connection::rank_installed(provider, found, &RUNNER)
        .into_iter()
        .map(|installed| installed.executable)
        .collect(),
    ),
    not_found @ ProgramLookup::NotFound { .. } => not_found,
  })
}

/// The copy among `found` (installed copies, as [`detect_agent`] lists them) that `program` names, as
/// the file system resolves both: through links, a junction or another spelling of the same path.
pub fn found_copy<'a>(found: &'a [PathBuf], program: &Path) -> Option<&'a Path> {
  let program = program.canonicalize().ok()?;
  found
    .iter()
    .find(|copy| copy.canonicalize().is_ok_and(|copy| copy == program))
    .map(PathBuf::as_path)
}

/// Installed copies of `program` (see `ConnectionKind::program_name`) found on the search path
/// (the `PATH`, and on macOS the login shell's and the usual install folders:
/// `orchester_exec::search_path`), in search order.
pub fn detect(program: &str) -> Vec<PathBuf> {
  match SearchPath::current().lookup(program) {
    ProgramLookup::Found(found) => found,
    ProgramLookup::NotFound { .. } => Vec::new(),
  }
}
