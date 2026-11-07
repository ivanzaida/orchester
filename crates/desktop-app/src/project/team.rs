//! The team's configuration of a shared project, read with its feed for the five libraries and their editors
//! (`orchester_workspace::sharing::team`): again whenever the shared project's cache changed, a version published here
//! was sent, or a library's resources were changed here or by automation.

use std::path::{Path, PathBuf};

use lurq::app::ctx::{Ctx, FutureStatus};
use orchester_workspace::{
  background,
  messages::{self, Message},
  sharing::{Servers, SharingError, team::TeamConfiguration},
  tasks::outside::{Outside, OutsideCounts},
};

use super::hold::ReadHold;

/// The library kinds whose changes move how the team's resources stand here.
const LIBRARIES: [Outside; 5] = [
  Outside::Pipelines,
  Outside::Profiles,
  Outside::Instructions,
  Outside::Skills,
  Outside::Tools,
];

/// What the feed knows of whether the project is shared. Whether a project is shared lives in its database, so a screen
/// that tells a local project from a team's waits for the first read instead of taking the unread for local.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamRead {
  /// Not read yet, or read again after a first read that failed.
  Unread,
  /// The first read failed; why.
  Failed(Message),
  /// Read: the team's configuration while the project is shared, `None` while it is local. A later reading that fails
  /// is logged and leaves this one.
  Read(Option<TeamConfiguration>),
}

impl lurq::app::component::DevtoolsInspectable for TeamRead {}

impl TeamRead {
  /// The team's configuration while the project is known to be shared.
  pub fn configuration(&self) -> Option<TeamConfiguration> {
    match self {
      Self::Read(team) => team.clone(),
      Self::Unread | Self::Failed(_) => None,
    }
  }
}

/// What the project at `root` has read of its team's configuration. `retry` counts the person's Try agains.
pub(super) fn read(
  ctx: &mut Ctx,
  (servers, root): (&Servers, &Path),
  (shared, outside, retry): (u64, OutsideCounts, u64),
  hold: &ReadHold,
) -> TeamRead {
  let libraries: u64 = LIBRARIES.iter().map(|kind| outside.of(*kind)).sum();
  let (servers, hold) = (servers.clone(), hold.clone());
  let deps: (PathBuf, u64, u64, u64) = (root.to_owned(), shared, libraries, retry);
  let load = ctx.future(deps, move |(root, ..)| {
    tracing::trace!(target: crate::app_log::READ_LOG, read = "team configuration");
    let (servers, hold) = (servers.clone(), hold.clone());
    async move {
      hold.released().await;
      // Logged, and kept as the failure of a first read; a later one leaves the configuration last read, and the store
      // refuses what a member may not do.
      background::run(move || servers.team_configuration(&root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the team's configuration of a shared project");
          match error {
            SharingError::Store(error) => messages::store_error(&error),
            _ => messages::interrupted(),
          }
        })
    }
  });
  let state = load.state().get();
  match (state.data, state.status, state.error) {
    (Some(team), ..) => TeamRead::Read(team),
    (None, FutureStatus::Rejected, Some(error)) => TeamRead::Failed(error),
    (None, ..) => TeamRead::Unread,
  }
}
