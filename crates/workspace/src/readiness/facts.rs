//! What this computer has, read into core's [`Here`].

use std::{fs, path::Path};

use orchester_core::{connection::ConnectionKind, id::ResourceId, sharing::Here, store::ConnectionStore};
use orchester_store::{McpServerProgram, ProjectDb, StoreError};

/// The agent connections and the secret values set by name. A local project has no secret by name, so it has none.
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
pub(super) fn with_programs(mut here: Here, programs: Vec<McpServerProgram>, servers: &[ResourceId]) -> Here {
  for program in programs
    .into_iter()
    .filter(|program| servers.contains(&program.resource))
  {
    let exists = is_file(&program.program);
    here = here.with_program(program.resource, program.program, exists);
  }
  here
}

fn is_file(path: &Path) -> bool {
  fs::metadata(path).is_ok_and(|found| found.is_file())
}
