//! How the page follows changes made outside it (the person's decision of 2026-10-15): while an agent works in the
//! run's worktree shown, its tree, its file and its versions are read again on each change the runner reports of
//! the project's runs and transcripts; the checkout, the base branch and a run's worktree are read again when
//! the project's merge and integration operations change; a commit never. Nothing polls: the page reads the
//! project feed's signals, and only the ones its version follows, so it renders for nothing else. A read again
//! keeps what was shown until it is done, and what it found the same stays the value shown ([`Steady`]), so the
//! selection, the scroll, the search and the open folders stay.

use orchester_workspace::{
  files::{FileList, FileView, Follow, Version},
  tasks::integration_jobs::ProjectJobState,
};
use parking_lot::Mutex;

use super::reads::{Steady, Wake};
use crate::project::ShownProject;

/// The page's following: what it last showed, and how often the integration operations changed.
#[derive(Default)]
pub struct Following {
  pub list: Steady<FileList>,
  pub file: Steady<FileView>,
  jobs: Mutex<(Option<ProjectJobState>, u64)>,
}

impl Following {
  /// What wakes the reads of `at` in `project` now, with Try again's count `retry`.
  pub fn wake(&self, project: &ShownProject, at: &Version, retry: u64) -> Wake {
    let feed = &project.feed;
    let tasks = feed.tasks.get();
    let follow = Follow::of(at, tasks.ready());
    let runs = match follow {
      Follow::RunChanges => feed.changes.get(),
      Follow::Integration | Follow::Never => 0,
    };
    let jobs = match follow {
      Follow::RunChanges | Follow::Integration => self.jobs(feed.job_state.get()),
      Follow::Never => 0,
    };
    Wake { retry, runs, jobs }
  }

  /// How often `state` changed since the page was made.
  fn jobs(&self, state: ProjectJobState) -> u64 {
    let mut seen = self.jobs.lock();
    if seen.0.as_ref() != Some(&state) {
      seen.1 += u64::from(seen.0.is_some());
      seen.0 = Some(state);
    }
    seen.1
  }
}
