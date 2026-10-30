//! The team's configuration of a shared project, read with its feed for the five libraries and their editors
//! (`orchester_workspace::sharing::team`): again whenever the shared project's cache changed, a version published here
//! was sent, or a library's resources were changed here or by automation.

use std::path::{Path, PathBuf};

use lurq::app::ctx::Ctx;
use orchester_workspace::{
  background, messages,
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

/// The team's configuration of the project at `root`: `None` until first read, then `Some(None)` while the project is
/// local. A failed reading is logged and leaves the last one.
pub(super) fn read(
  ctx: &mut Ctx,
  (servers, root): (&Servers, &Path),
  (shared, outside): (u64, OutsideCounts),
  hold: &ReadHold,
) -> Option<Option<TeamConfiguration>> {
  let libraries: u64 = LIBRARIES.iter().map(|kind| outside.of(*kind)).sum();
  let (servers, hold) = (servers.clone(), hold.clone());
  let deps: (PathBuf, u64, u64) = (root.to_owned(), shared, libraries);
  let load = ctx.future(deps, move |(root, ..)| {
    tracing::trace!(target: crate::app_log::READ_LOG, read = "team configuration");
    let (servers, hold) = (servers.clone(), hold.clone());
    async move {
      hold.released().await;
      // Only logged: the libraries keep the configuration last read, and the store refuses what a member may not do.
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
  load.state().get().data
}
