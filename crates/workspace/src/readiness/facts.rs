//! What this computer has, read into core's [`Here`].

use std::{fs, path::Path};

use orchester_core::{
  connection::ConnectionKind,
  id::{ConnectionId, ResourceId, SecretId},
  sharing::Here,
  store::ConnectionStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::programs::McpPrograms;

/// The agent connections and the team secrets that have a value, by name. A local project has no secret by name, so it
/// has none. "Has a value" is a row in `team_secret_values`: the keychain is not read, so a handle whose keychain value
/// was removed outside Orchester still counts.
pub(super) fn base(db: &ProjectDb) -> Result<Here, StoreError> {
  let mut here = Here::nothing();
  for connection in db.connections()? {
    if let ConnectionKind::Agent(provider) = connection.kind() {
      here = here.with_connection(connection.id(), *provider);
    }
  }
  for name in db.team_secret_names_set()? {
    here = here.with_secret(name);
  }
  Ok(here)
}

/// The program path of each of `servers` that has one, with whether its file is there now. A path that cannot be read
/// counts as no file: the person is asked to choose again either way.
pub(super) fn with_programs(mut here: Here, programs: &McpPrograms, servers: &[ResourceId]) -> Here {
  for (server, path) in programs.iter().filter(|(server, _)| servers.contains(server)) {
    here = here.with_program(server, path.to_path_buf(), is_file(path));
  }
  here
}

fn is_file(path: &Path) -> bool {
  fs::metadata(path).is_ok_and(|found| found.is_file())
}

/// What looking at this computer found for a local resource's needs.
#[derive(Clone, Copy, Debug)]
pub(super) enum Found {
  /// The program of the local MCP server is a file a run starts.
  Program(ConnectionId),
  /// The program of the local MCP server is a file, but a run refuses to start it where it leads now.
  ProgramMoved(ConnectionId),
  /// The project secret has a value in the keychain.
  SecretValue(SecretId),
}

impl Found {
  pub(super) fn add_to(self, here: Here) -> Here {
    match self {
      Self::Program(server) => here.with_local_program(server),
      Self::ProgramMoved(server) => here.with_local_program_moved(server),
      Self::SecretValue(secret) => here.with_local_secret(secret),
    }
  }
}
