//! A local project archives what it has instead of removing it (ORC-842, decided 2026-11-08; the local twin of
//! `docs/design/team-configuration.md` decision 11): a profile, instruction, skill, tool or MCP server leaves the lists
//! and pickers, the runs that use it stop, and it can be restored as it was. Archiving is never refused for being used:
//! the confirmation lists what uses it ([`crate::used_by`]) and the runs that stop. Pipeline steps keep naming what is
//! archived, and readiness then says the pipeline is not ready with the archived resource named
//! ([`crate::readiness::Undecided::RunsArchived`]). It stops work, so it is a person's alone: automation is refused
//! before anything changes.

use std::path::Path;

use orchester_core::{
  id::{ConnectionId, ResourceId, RunId},
  resource::ResourceKind,
  run::Answerer,
  time::Timestamp,
};
use orchester_store::{Archived, ProjectDb, StoreError};

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

/// Why something was not archived; nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotArchived {
  /// Automation asked: it stops runs, so only a person archives.
  PersonOnly,
  Failed(Message),
}

impl NotArchived {
  /// What a person-only refusal says.
  pub fn person_only_message() -> Message {
    Message::new("appMcp.errors.teamArchivePersonOnly")
  }
}

/// Archives `what` in the project at `root`, as `by` asks, and cancels the runs that use it. The number of runs
/// stopped. A run that cannot be cancelled is logged and not counted ([`sync::stop`]).
pub async fn archive(runners: &Runners, root: &Path, what: Archivable, by: Answerer) -> Result<usize, NotArchived> {
  if by == Answerer::Automation {
    return Err(NotArchived::PersonOnly);
  }
  let owned = root.to_owned();
  let pinning = background::run(move || mark(&owned, what))
    .await
    .map_err(NotArchived::Failed)?
    .map_err(NotArchived::Failed)?;
  let stopped = sync::stop(runners, root, pinning.0, pinning.1).await;
  if let Archivable::McpServer(server) = what {
    let (owned, runs) = (root.to_owned(), stopped.clone());
    background::run(move || note_stops(&owned, server, &runs))
      .await
      .map_err(NotArchived::Failed)?
      .map_err(NotArchived::Failed)?;
  }
  Ok(stopped.len())
}

/// Notes that the archive of `server` stopped `runs`, so its panel can say which.
fn note_stops(root: &Path, server: ConnectionId, runs: &[RunId]) -> Result<(), Message> {
  let store = |error: &StoreError| messages::store_error(error);
  let mut db = ProjectDb::open(root).map_err(|error| store(&error))?;
  runs
    .iter()
    .try_for_each(|run| db.record_mcp_server_stop(*run, server))
    .map_err(|error| store(&error))
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

/// Marks `what` archived and returns the runs that used it, read first, with what the stop records.
fn mark(root: &Path, what: Archivable) -> Result<(Pinning, Option<Archived>), Message> {
  let store = |error: &StoreError| messages::store_error(error);
  let found = match what {
    Archivable::Resource(id) => sync::pinning(root, id),
    Archivable::McpServer(id) => sync::pinning_mcp_server(root, id),
  };
  let pinning = found
    .map_err(|error| match error {
      ArchiveError::Store(error) => store(&error),
      ArchiveError::Runs(_) => Message::new("localArchive.runsUnreadable"),
    })?
    .ok_or_else(|| Message::new("localArchive.gone"))?;
  let mut db = ProjectDb::open(root).map_err(|error| store(&error))?;
  let now = Timestamp::now();
  let archived = match what {
    Archivable::Resource(id) => {
      db.archive_local_resource(id, now).map_err(|error| store(&error))?;
      Some(Archived {
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
}
