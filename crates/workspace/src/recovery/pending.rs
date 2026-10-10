//! What the person has and has not decided about interrupted runs, kept across starts in the application
//! database. A project with undecided runs keeps since when they were interrupted, so its notice comes
//! back at every start, also after a clean end, until the person decides. A project keeps when the person
//! last decided, so runs decided on are not offered again after a later Quit now or crash.

use std::collections::HashMap;

use orchester_core::{id::ProjectId, time::Timestamp};

use crate::workspace::Workspace;

const PENDING: &str = "recovery.pending.";
const DECIDED: &str = "recovery.decided.";

/// Each project's marks.
#[derive(Clone, Debug, Default)]
pub(super) struct Marks {
  /// Since when its undecided runs were interrupted.
  pub pending: HashMap<ProjectId, Timestamp>,
  /// When the person last decided about its runs.
  pub decided: HashMap<ProjectId, Timestamp>,
}

/// The marks of every project. What cannot be read is logged and left out: the runs then stay
/// interrupted on their task pages, and a notice shows only after a Quit now or a crash.
pub(super) fn read(workspace: &Workspace) -> Marks {
  Marks {
    pending: times(workspace, PENDING),
    decided: times(workspace, DECIDED),
  }
}

fn times(workspace: &Workspace, prefix: &str) -> HashMap<ProjectId, Timestamp> {
  let keys = match workspace.ui_state_keys(prefix) {
    Ok(keys) => keys,
    Err(error) => {
      tracing::warn!(%error, prefix, "cannot read the interrupted runs' marks");
      return HashMap::new();
    }
  };
  keys
    .into_iter()
    .filter_map(|key| {
      // A key not written by `set` is as if there were none.
      let project = key.strip_prefix(prefix)?.parse().ok()?;
      let stored = match workspace.ui_state(&key) {
        Ok(stored) => stored?,
        Err(error) => {
          tracing::warn!(%error, key, "cannot read an interrupted runs' mark");
          return None;
        }
      };
      // As above: a value not written by `set` is as if there were none.
      let millis = stored.parse().ok()?;
      Some((project, Timestamp::from_unix_millis(millis)))
    })
    .collect()
}

/// `project` has undecided interrupted runs, interrupted at `since` or later.
pub(super) fn keep(workspace: &Workspace, project: ProjectId, since: Timestamp) {
  set(workspace, &format!("{PENDING}{project}"), since);
}

/// The person decided about `project`'s interrupted runs at `at`: none of them is offered again.
pub(super) fn decide(workspace: &Workspace, project: ProjectId, at: Timestamp) {
  set(workspace, &format!("{DECIDED}{project}"), at);
  forget(workspace, project);
}

/// `project` has no undecided runs (they were continued from their task pages, or decided).
pub(super) fn forget(workspace: &Workspace, project: ProjectId) {
  if let Err(error) = workspace.remove_ui_state(&format!("{PENDING}{project}")) {
    // The mark brings the notice back once more, with what is still interrupted then.
    tracing::warn!(%project, %error, "cannot forget the undecided interrupted runs");
  }
}

/// Losing a mark only changes when a notice shows: the runs stay interrupted on their task pages.
fn set(workspace: &Workspace, key: &str, at: Timestamp) {
  if let Err(error) = workspace.set_ui_state(key, &at.unix_millis().to_string()) {
    tracing::warn!(key, %error, "cannot keep an interrupted runs' mark");
  }
}
