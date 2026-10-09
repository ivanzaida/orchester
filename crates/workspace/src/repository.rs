//! The project's GitHub repository on this computer (`docs/design/orchester-workspace.md`,
//! "GitHub"): detecting it from the project folder with the programs found here, and recording
//! what detection found — the GitHub connection for the repository's host and the project's
//! link. Every function blocks on git, `gh` or SQLite; the UI calls them off the UI thread.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{ConnectionError, GH_PROGRAM, GitHubConnection, GitHubHost, GitHubLogin},
  id::ConnectionId,
  name::{Name, NameError},
  repository::{DetectRequest, Detection, GhProgram, RemoteName, RepositoryLink},
  store::{ConnectionStore, RepositoryStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError, is_repository_root};
use thiserror::Error;

use crate::{
  connections::{data::github_connections, probe},
  messages::{self, Message},
};

/// The `git` program, looked up on the `PATH` like the runner does.
const GIT: &str = "git";

/// Where and with which programs to detect the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectTarget {
  pub repo_root: PathBuf,
  /// The project's GitHub connections: their hosts count as GitHub and their `gh` is asked.
  pub connections: Vec<GitHubConnection>,
  /// A `gh` the user chose for github.com while the project has no connection there; otherwise
  /// the first `gh` on the `PATH` is asked.
  pub program: Option<PathBuf>,
}

/// Detects the GitHub repository of the folder. A folder that is not the top of a git repository
/// cannot hold a project, whatever git says about a parent folder.
pub fn detect(target: &DetectTarget) -> Detection {
  if !is_repository_root(&target.repo_root) {
    return Detection::NotAGitRepository;
  }
  let programs = programs(target);
  let origin = RemoteName::origin();
  probe::detect_repository(&DetectRequest {
    repo_root: &target.repo_root,
    git: Path::new(GIT),
    remote: &origin,
    programs: &programs,
  })
}

/// The hosts of the project's connections with their programs, and github.com.
fn programs(target: &DetectTarget) -> Vec<GhProgram> {
  let mut programs: Vec<GhProgram> = target
    .connections
    .iter()
    .map(|connection| GhProgram {
      host: connection.host().clone(),
      executable: Some(connection.executable().to_path_buf()),
    })
    .collect();
  let github_com = GitHubHost::github_com();
  if !programs.iter().any(|program| program.host == github_com) {
    let executable = target
      .program
      .clone()
      .or_else(|| probe::detect(GH_PROGRAM).into_iter().next());
    programs.push(GhProgram {
      host: github_com,
      executable,
    });
  }
  programs
}

#[derive(Debug, Error)]
pub enum LinkError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Connection(#[from] ConnectionError),
  #[error("the connection name is not valid: {0}")]
  Name(#[from] NameError),
  /// Detection did not find a repository the account can read through a working `gh`.
  #[error("the detected repository cannot be linked")]
  NotLinkable,
}

impl LinkError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::Connection(_) | Self::Name(_) => Message::new("errors.storage"),
      Self::NotLinkable => Message::new("errors.repositoryNotLinkable"),
    }
  }
}

/// Makes sure the project has a GitHub connection for the detected repository's host, named
/// `name` when it is created, and stores the check detection ran on it. `None` when detection
/// found no GitHub CLI to connect through.
pub fn record_connection<S>(
  store: &mut S,
  detection: &Detection,
  name: &str,
  now: Timestamp,
) -> Result<Option<GitHubConnection>, LinkError>
where
  S: ConnectionStore<Error = StoreError>,
{
  let Some(found) = detection.github() else {
    return Ok(None);
  };
  let (Some(program), Some(check)) = (found.program(), found.check()) else {
    return Ok(None);
  };
  let host = found.repo().host();
  let existing = github_connections(store.connections()?)
    .into_iter()
    .find(|connection| connection.host() == host);
  match existing {
    Some(mut connection) => {
      // Detection asked this connection's program, so its check is the connection's latest.
      if connection.executable() == program {
        connection.record_check(check.clone());
        store.save_connection(&connection)?;
      }
      Ok(Some(connection))
    }
    None if check.result.is_verified() => {
      let mut connection = GitHubConnection::new(
        ConnectionId::generate(),
        Name::new(name)?,
        host.clone(),
        program.to_path_buf(),
        now,
      )?;
      connection.record_check(check.clone());
      store.create_connection(&connection)?;
      Ok(Some(connection))
    }
    None => Ok(None),
  }
}

/// Records the connection and links the project to the detected repository. The project
/// expects `expected`, or, when it expects nobody yet, the account `gh` acts as.
pub fn link<S>(
  store: &mut S,
  detection: &Detection,
  expected: Option<&GitHubLogin>,
  name: &str,
  now: Timestamp,
) -> Result<RepositoryLink, LinkError>
where
  S: ConnectionStore<Error = StoreError> + RepositoryStore<Error = StoreError>,
{
  adopt(store, detection, expected, name, now)?.ok_or(LinkError::NotLinkable)
}

/// What a new project takes over from detection in setup: the GitHub connection for the host
/// when `gh` answered there, and the link when the account could read the repository.
pub fn adopt<S>(
  store: &mut S,
  detection: &Detection,
  expected: Option<&GitHubLogin>,
  name: &str,
  now: Timestamp,
) -> Result<Option<RepositoryLink>, LinkError>
where
  S: ConnectionStore<Error = StoreError> + RepositoryStore<Error = StoreError>,
{
  let Some(connection) = record_connection(store, detection, name, now)? else {
    return Ok(None);
  };
  let Some(link) = detection.link(connection.id(), expected, now) else {
    return Ok(None);
  };
  store.save_repository_link(&link)?;
  Ok(Some(link))
}

pub fn adopt_in(
  repo_root: &Path,
  detection: &Detection,
  expected: Option<&GitHubLogin>,
  name: &str,
) -> Result<Option<RepositoryLink>, LinkError> {
  adopt(
    &mut ProjectDb::open(repo_root)?,
    detection,
    expected,
    name,
    Timestamp::now(),
  )
}

/// Check again for a linked project: records the check and, when detection read the linked repository
/// through the linked connection, refreshes the link (name, default branch, permission). The
/// expected account stays; only *Use signed-in account* changes it.
pub fn refresh<S>(store: &mut S, detection: &Detection, name: &str, now: Timestamp) -> Result<(), LinkError>
where
  S: ConnectionStore<Error = StoreError> + RepositoryStore<Error = StoreError>,
{
  // An unlinked project records nothing: linking adds the connection when the user asks.
  let Some(link) = store.repository_link()? else {
    return Ok(());
  };
  let Some(connection) = record_connection(store, detection, name, now)? else {
    return Ok(());
  };
  if connection.id() != link.connection() || !links_same_repository(detection, &link) {
    return Ok(());
  }
  if let Some(fresh) = detection.link(connection.id(), Some(link.expected_login()), now) {
    store.save_repository_link(&fresh)?;
  }
  Ok(())
}

/// Whether detection found the linked repository: under the remote's name or, after a rename
/// on GitHub, under the name GitHub reports now.
pub fn links_same_repository(detection: &Detection, link: &RepositoryLink) -> bool {
  detection
    .github()
    .is_some_and(|found| found.repo() == link.repo() || found.view().is_some_and(|view| &view.repo == link.repo()))
}

pub fn link_in(
  repo_root: &Path,
  detection: &Detection,
  expected: Option<&GitHubLogin>,
  name: &str,
) -> Result<RepositoryLink, LinkError> {
  link(
    &mut ProjectDb::open(repo_root)?,
    detection,
    expected,
    name,
    Timestamp::now(),
  )
}

pub fn refresh_in(repo_root: &Path, detection: &Detection, name: &str) -> Result<(), LinkError> {
  refresh(&mut ProjectDb::open(repo_root)?, detection, name, Timestamp::now())
}
