//! A local project archives what it has instead of removing it (ORC-842, decided 2026-11-08; the local twin of
//! `docs/design/team-configuration.md` decision 11): a profile, instruction, skill, tool or MCP server leaves the lists
//! and pickers, the runs that use it stop, and it can be restored as it was. Archiving is never refused for being used:
//! the confirmation lists what uses it ([`crate::used_by`]) and the runs that stop. Pipeline steps keep naming what is
//! archived, and readiness then says the pipeline is not ready with the archived resource named
//! ([`crate::readiness::Undecided::RunsArchived`]). It stops work, so it is a person's alone: automation is refused
//! before anything changes.

use std::path::Path;

use orchester_core::{
  id::{ConnectionId, ResourceId},
  resource::ResourceKind,
  time::Timestamp,
};
use orchester_store::{Archived as ArchivedResource, ProjectDb, StoreError};

use crate::{
  background,
  messages::{self, Message},
  runner::Runners,
  sharing::sync::{self, ArchiveError, Pinning},
  tasks::outside::Outside,
};

/// What a local project archives: a versioned resource, or a local MCP server, which is a connection with its own id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Archivable {
  Resource(ResourceId),
  McpServer(ConnectionId),
}

impl Archivable {
  /// What an archive or a restore changes besides the thing itself: its own list, the pipelines whose readiness names
  /// it, and, for an MCP server, the profiles that choose it.
  pub const fn changes(self, kind: Option<ResourceKind>) -> &'static [Outside] {
    match (self, kind) {
      (Self::McpServer(_), _) => &[Outside::Connections, Outside::Profiles, Outside::Pipelines],
      (Self::Resource(_), Some(ResourceKind::AgentProfile)) => &[Outside::Profiles, Outside::Pipelines],
      (Self::Resource(_), Some(ResourceKind::Tool)) => &[Outside::Tools, Outside::Pipelines],
      (Self::Resource(_), Some(ResourceKind::Instruction)) => &[Outside::Instructions],
      (Self::Resource(_), Some(ResourceKind::Skill)) => &[Outside::Skills],
      (Self::Resource(_), _) => &[Outside::Pipelines],
    }
  }
}

/// What archiving stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Archived {
  /// The runs it cancelled.
  pub stopped: usize,
  /// The runs that use it that the runner could not cancel: they keep working, on what is archived.
  pub left_running: usize,
}

impl Archived {
  /// What to tell the person when some runs that use what was archived could not be stopped; `None` when all were.
  pub fn left_running_message(self) -> Option<Message> {
    (self.left_running > 0)
      .then(|| Message::new("localArchive.runsLeftRunning").with("count", self.left_running.to_string()))
  }
}

/// Archives `what` in the project at `root` and cancels the runs that use it. Stopping work is a person's alone: the
/// caller asks only after a person confirmed, and refuses automation before it comes here. The archive and the runs it
/// stops are read in one transaction ([`mark`]), and a run that starts after it is refused by the store, so none is
/// missed. `Err` says why nothing was archived.
pub async fn archive(runners: &Runners, root: &Path, what: Archivable) -> Result<Archived, Message> {
  let owned = root.to_owned();
  let (pinning, archived) = background::run(move || mark(&owned, what)).await??;
  let server = match what {
    Archivable::McpServer(server) => Some(server),
    Archivable::Resource(_) => None,
  };
  let stopped = sync::stop(runners, root, pinning, (archived, server)).await;
  Ok(Archived {
    stopped: stopped.runs.len(),
    left_running: stopped.left_running,
  })
}

/// Brings `what` back as it was.
pub async fn restore(root: &Path, what: Archivable) -> Result<(), Message> {
  let owned = root.to_owned();
  background::run(move || {
    let mut db = ProjectDb::open(&owned).map_err(|error| messages::store_error(&error))?;
    match what {
      Archivable::Resource(id) => db.restore_local_resource(id),
      Archivable::McpServer(id) => db.restore_mcp_server(id),
    }
    .map(drop)
    .map_err(|error| messages::store_error(&error))
  })
  .await?
}

/// Marks `what` archived and returns the runs that use it, with what the stop records: both in one write transaction,
/// so a run cannot start between reading them and archiving.
fn mark(root: &Path, what: Archivable) -> Result<(Pinning, Option<ArchivedResource>), Message> {
  let store = |error: &StoreError| messages::store_error(error);
  let mut db = ProjectDb::open(root).map_err(|error| store(&error))?;
  db.with_write_transaction(
    |db| {
      let found = match what {
        Archivable::Resource(id) => sync::pinning_in(db, root, (id, None)),
        Archivable::McpServer(id) => sync::pinning_mcp_server_in(db, root, id),
      };
      let pinning = found
        .map_err(|error| match error {
          ArchiveError::Store(error) => store(&error),
          ArchiveError::Runs(_) => Message::new("localArchive.runsUnreadable"),
        })?
        .ok_or_else(|| Message::new("localArchive.gone"))?;
      let now = Timestamp::now();
      let archived = match what {
        Archivable::Resource(id) => {
          db.archive_local_resource(id, now).map_err(|error| store(&error))?;
          Some(ArchivedResource {
            id,
            kind: pinning.kind(),
            by: None,
          })
        }
        Archivable::McpServer(id) => {
          db.archive_mcp_server(id, now).map_err(|error| store(&error))?;
          None
        }
      };
      Ok((pinning, archived))
    },
    |error| store(&error),
    |_, error| store(&error),
  )
}
