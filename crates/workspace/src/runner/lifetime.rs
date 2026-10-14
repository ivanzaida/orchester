//! How long a project's runner lives (design doc, "Tabs" › "Runners"): a project is open while at
//! least one window tab shows it, or while its runner has work — a run in progress or waiting for
//! an agent slot, or a checkpoint being saved. Once no tab shows a project, its runner lingers:
//! it is asked to stop whenever something about it changes, and stops only when it answers that
//! it has no work (`Command::StopIfIdle`), so nothing it does is interrupted. A tab showing the
//! project again ends the lingering. Closing the window still stops every runner, as before.

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
  time::Duration,
};

use tokio::task::JoinHandle;

use super::{Runners, idle::NotAsked};
use crate::workspace::OpenProject;

/// How long a lingering runner waits for a change before it asks again: its work usually ends
/// with a report, but a runner that is still starting or reconnecting reports nothing new.
const LINGER_RECHECK: Duration = Duration::from_secs(5);

/// The task asking a runner no tab shows to stop; dropping it ends the lingering.
pub(super) struct Lingering(JoinHandle<()>);

impl Drop for Lingering {
  fn drop(&mut self) {
    self.0.abort();
  }
}

/// Holds a project's runner open while a request that gives it work (continuing interrupted runs)
/// is under way, so a lingering runner does not stop before the work arrives.
pub(super) struct Pin {
  runners: Runners,
  root: PathBuf,
}

impl Drop for Pin {
  fn drop(&mut self) {
    if let Some(entry) = self.runners.inner.projects.lock().get_mut(&self.root) {
      entry.pins = entry.pins.saturating_sub(1);
    }
  }
}

impl Runners {
  /// The projects the window's tabs show. Each one's runner starts unless it runs; the runner of a
  /// project no tab shows any more stops once it has no work.
  pub fn keep_open(&self, projects: &[OpenProject]) {
    let tabbed: HashSet<PathBuf> = projects.iter().map(|project| project.repo_root.clone()).collect();
    self.inner.tabbed.lock().clone_from(&tabbed);
    for project in projects {
      self.ensure(&project.repo_root, project.project.name().as_str());
    }
    self.linger_untabbed();
  }

  /// Whether a window tab shows project `root`.
  pub(super) fn tabbed(&self, root: &Path) -> bool {
    self.inner.tabbed.lock().contains(root)
  }

  /// Starts lingering for every runner whose project no tab shows, and ends it for the others.
  pub(super) fn linger_untabbed(&self) {
    let tabbed = self.inner.tabbed.lock().clone();
    let mut projects = self.inner.projects.lock();
    for (root, entry) in projects.iter_mut() {
      if tabbed.contains(root) {
        entry.lingering = None;
      } else if entry.lingering.is_none() {
        let task = self.inner.runtime.spawn(self.clone().linger(root.clone()));
        entry.lingering = Some(Lingering(task));
      }
    }
  }

  /// Keeps project `root`'s runner open until the pin is dropped.
  pub(super) fn pin(&self, root: &Path) -> Pin {
    if let Some(entry) = self.inner.projects.lock().get_mut(root) {
      entry.pins += 1;
    }
    Pin {
      runners: self.clone(),
      root: root.to_path_buf(),
    }
  }

  /// Asks project `root`'s runner to stop whenever its work or runs change, until it stopped.
  async fn linger(self, root: PathBuf) {
    let mut work = self.work_changes();
    let mut runs = self.changes_of(&root);
    loop {
      let asked = match self.ask_idle(&root) {
        Ok(asked) => Some(asked),
        Err(NotAsked::Gone) => return,
        Err(NotAsked::Held) => None,
      };
      if let Some((generation, mut answer)) = asked
        && matches!(
          answer.wait_for(Option::is_some).await.map(|stopped| *stopped),
          Ok(Some(true))
        )
      {
        self.retire_idle(&root, generation).await;
        return;
      }
      tokio::select! {
        _ = work.changed() => {}
        _ = runs.changed() => {}
        () = tokio::time::sleep(LINGER_RECHECK) => {}
      }
    }
  }
}
