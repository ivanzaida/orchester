//! Which agent CLI the start page says is not found (`docs/design/startup.md`, decision 9). With listed projects: one
//! a connection runs whose program file is gone, with no copy on the search path. With none: neither CLI on the search
//! path and no file chosen before, which names Claude Code, since a first start has no connection to name a CLI. One
//! CLI at a time, Claude Code first, the order Connections lists them. Everything here blocks; the UI calls it off the
//! UI thread.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{AgentProvider, Connection, ConnectionKind},
  id::{ConnectionId, ProjectId},
  name::Name,
  permission,
  store::ConnectionStore,
};
use orchester_exec::search_path::{ProgramLookup, SearchBasis, SearchFolder, SearchPath};
use orchester_store::{ProjectDb, StoreError};

use super::probe;
use crate::workspace::{ProjectEntry, Workspace};

/// The CLIs a connection runs on, in the order the start page names them.
const CLIS: [AgentProvider; 2] = [AgentProvider::ClaudeCodeCli, AgentProvider::CodexCli];
/// Where the application database keeps the file chosen for a CLI that no connection names yet.
const REMEMBERED: &str = "orchester-app.agentProgram.";

/// A connection whose program file is gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedingConnection {
  pub project: ProjectId,
  /// The project's name, for what a failure says.
  pub name: Name,
  pub repo_root: PathBuf,
  pub connection: ConnectionId,
}

/// The CLI not found, where it was looked for, and the connections a chosen file fixes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingCli {
  pub provider: AgentProvider,
  /// The name of its program (`claude`).
  pub program: &'static str,
  /// The folders searched, as a person reads them; none when the search path has none.
  pub searched: Vec<String>,
  pub basis: SearchBasis,
  /// None at a first start, where a chosen file waits for the first connection.
  pub needing: Vec<NeedingConnection>,
}

/// What the start page warns of for `entries`, the listed projects, searching `search`, which looked at `basis`.
pub fn missing_cli_on(
  workspace: &Workspace,
  entries: &[ProjectEntry],
  search: &SearchPath,
  basis: &SearchBasis,
) -> Result<Option<MissingCli>, StoreError> {
  if entries.is_empty() {
    return first_start(workspace, search, basis);
  }
  let connections = stored_connections(entries);
  for provider in CLIS {
    let needing = needing(provider, &connections);
    if needing.is_empty() {
      continue;
    }
    if let Some(missing) = not_found(provider, search, basis, needing) {
      return Ok(Some(missing));
    }
  }
  Ok(None)
}

/// With nothing listed, Claude Code when neither CLI is on the search path and no file was chosen for either.
fn first_start(
  workspace: &Workspace,
  search: &SearchPath,
  basis: &SearchBasis,
) -> Result<Option<MissingCli>, StoreError> {
  for provider in CLIS {
    if remembered_program(workspace, provider)?.is_some() {
      return Ok(None);
    }
  }
  if CLIS
    .into_iter()
    .any(|provider| not_found(provider, search, basis, Vec::new()).is_none())
  {
    return Ok(None);
  }
  Ok(not_found(AgentProvider::ClaudeCodeCli, search, basis, Vec::new()))
}

/// `provider`'s CLI when `search` holds no copy of it.
fn not_found(
  provider: AgentProvider,
  search: &SearchPath,
  basis: &SearchBasis,
  needing: Vec<NeedingConnection>,
) -> Option<MissingCli> {
  let program = provider.program_name()?;
  let ProgramLookup::NotFound { searched } = search.lookup(program) else {
    return None;
  };
  Some(MissingCli {
    provider,
    program,
    searched: searched.iter().map(SearchFolder::shown).collect(),
    basis: basis.clone(),
    needing,
  })
}

/// The connections of each listed project that can be read. A project whose folder is gone or whose data cannot be
/// read says so in its own row, so it warns of nothing here.
fn stored_connections(entries: &[ProjectEntry]) -> Vec<(&ProjectEntry, Vec<Connection>)> {
  entries
    .iter()
    .filter(|entry| !entry.folder_missing)
    .filter_map(|entry| {
      let read = ProjectDb::open_read_only(&entry.project.repo_root).and_then(|db| db.connections());
      match read {
        Ok(connections) => Some((entry, connections)),
        // Opening the project migrates its data; until then its connections cannot be read.
        Err(StoreError::SchemaTooOld { .. }) => None,
        Err(error) => {
          tracing::warn!(%error, repo_root = %entry.project.repo_root.display(), "cannot read a listed project's connections");
          None
        }
      }
    })
    .collect()
}

/// The connections among `connections` that run `provider`'s CLI from a file that is gone.
fn needing(provider: AgentProvider, connections: &[(&ProjectEntry, Vec<Connection>)]) -> Vec<NeedingConnection> {
  connections
    .iter()
    .flat_map(|(entry, connections)| connections.iter().map(move |connection| (*entry, connection)))
    .filter(|(_, connection)| *connection.kind() == ConnectionKind::Agent(provider))
    .filter(|(_, connection)| connection.executable().is_some_and(gone))
    .map(|(entry, connection)| NeedingConnection {
      project: entry.project.id,
      name: entry.project.name.clone(),
      repo_root: entry.project.repo_root.clone(),
      connection: connection.id(),
    })
    .collect()
}

/// Whether the file `program` is gone. A path of another machine is not looked at, as reading it hands that server the
/// person's credentials, so it does not count as gone.
fn gone(program: &Path) -> bool {
  permission::stays_local(program).is_ok() && !program.is_file()
}

/// The copies of `provider`'s CLI a new connection can run: those on the search path, newest first, then the file
/// chosen on the start page if it is there and is not one of them.
pub fn known_copies(workspace: &Workspace, provider: AgentProvider) -> KnownCopies {
  let mut copies = probe::detect_agent(provider);
  let on_search_path = copies.len();
  match remembered_program(workspace, provider) {
    Ok(Some(chosen)) if probe::found_copy(&copies, &chosen).is_none() => copies.push(chosen),
    Ok(_) => {}
    // The copies on the search path are still offered; only the one chosen earlier is not.
    Err(error) => tracing::warn!(%error, "cannot read the program file chosen for a CLI"),
  }
  KnownCopies { copies, on_search_path }
}

/// The copies of a CLI a new connection can run, and how many of them are on the search path: the first ones.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KnownCopies {
  pub copies: Vec<PathBuf>,
  pub on_search_path: usize,
}

/// The file chosen on the start page for `provider`'s CLI, if there is one and it is still there.
fn remembered_program(workspace: &Workspace, provider: AgentProvider) -> Result<Option<PathBuf>, StoreError> {
  let Some(program) = provider.program_name() else {
    return Ok(None);
  };
  let stored = workspace.ui_state(&remembered_key(program))?;
  // Only a file of this machine: the person chose one, but a link on its way can lead elsewhere by now.
  Ok(
    stored
      .map(PathBuf::from)
      .filter(|file| permission::stays_local(file).is_ok() && file.is_file()),
  )
}

/// The key the file chosen for `program` is kept under.
pub(super) fn remembered_key(program: &str) -> String {
  format!("{REMEMBERED}{program}")
}
