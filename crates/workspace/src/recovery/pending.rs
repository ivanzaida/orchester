//! Interrupted runs the person has not decided on yet, kept across starts in the application database:
//! a recovery notice stays until the person continues the runs or leaves them, also after a clean
//! restart. Each project with undecided runs keeps since when its runs were interrupted.

use std::collections::HashMap;

use orchester_core::{id::ProjectId, time::Timestamp};

use crate::workspace::Workspace;

const PREFIX: &str = "recovery.pending.";

fn key(project: ProjectId) -> String {
  format!("{PREFIX}{project}")
}

/// The projects with undecided interrupted runs, and since when those runs were interrupted. What
/// cannot be read is logged and left out: its notice then shows only after a Quit now or a crash.
pub(super) fn read(workspace: &Workspace) -> HashMap<ProjectId, Timestamp> {
  let keys = match workspace.ui_state_keys(PREFIX) {
    Ok(keys) => keys,
    Err(error) => {
      tracing::warn!(%error, "cannot read the undecided interrupted runs");
      return HashMap::new();
    }
  };
  keys
    .into_iter()
    // A mark that cannot be read is as if there were none: the runs stay interrupted on their task pages.
    .filter_map(|key| {
      let project = key.strip_prefix(PREFIX)?.parse().ok()?;
      let since = workspace.ui_state(&key).ok().flatten()?.parse().ok()?;
      Some((project, Timestamp::from_unix_millis(since)))
    })
    .collect()
}

/// `project` has undecided interrupted runs, interrupted at `since` or later. Losing the mark only means
/// the notice does not come back after a clean restart; the runs stay interrupted on their task pages.
pub(super) fn keep(workspace: &Workspace, project: ProjectId, since: Timestamp) {
  if let Err(error) = workspace.set_ui_state(&key(project), &since.unix_millis().to_string()) {
    tracing::warn!(%project, %error, "cannot keep the undecided interrupted runs");
  }
}

/// The person decided on `project`'s interrupted runs. A mark left behind brings the notice back once
/// more after a restart, with what is still interrupted then.
pub(super) fn settle(workspace: &Workspace, project: ProjectId) {
  if let Err(error) = workspace.remove_ui_state(&key(project)) {
    tracing::warn!(%project, %error, "cannot forget the decided interrupted runs");
  }
}
