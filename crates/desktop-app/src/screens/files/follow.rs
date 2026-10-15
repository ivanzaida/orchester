//! How the page follows changes made outside it (the person's decision of 2026-10-15): while an agent works in the
//! run's worktree shown, its tree and its file are read again on each change the runner reports of the project's
//! runs and transcripts; the checkout, the base branch and a run's worktree, and the versions the page offers, are
//! read again when the project's merge and integration operations change; a commit never. Nothing polls: the page reads
//! the project feed's signals, and only the ones its version follows, so it renders for nothing else. A read again
//! keeps what was shown until it is done, and what it found the same stays the value shown ([`Steady`]), so the
//! selection, the scroll, the search and the open folders stay. The runner's changes, many in a row while an
//! agent works, wake the reads at most once per [`COALESCE`].

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::app::ctx::Ctx;
use orchester_workspace::{
  files::{FileList, FileView, Follow, Version},
  tasks::integration_jobs::ProjectJobState,
};
use parking_lot::Mutex;

use super::reads::{FileReads, Steady, Wake};
use crate::project::ShownProject;

/// The runner reports many changes in a row while an agent works: they wake the reads at most once per this,
/// with the latest of them.
const COALESCE: Duration = Duration::from_millis(500);

/// The page's following: what it last showed, how often the integration operations changed, and when the
/// runner's changes last woke the reads.
#[derive(Default)]
pub struct Following {
  pub list: Steady<FileList>,
  pub file: Steady<FileView>,
  pub reads: Arc<FileReads>,
  jobs: Mutex<(Option<ProjectJobState>, u64)>,
  runs_woke: Arc<Mutex<Option<Instant>>>,
  /// The runner's count of changes when the page started following them: what it reads then is up to it.
  runs_from: Mutex<Option<u64>>,
}

impl Following {
  /// What wakes the reads of `at` in `project` now, with Try again's count `retry`.
  pub fn wake(&self, ctx: &mut Ctx, project: &ShownProject, (at, retry): (&Version, u64)) -> Wake {
    let feed = &project.feed;
    let tasks = feed.tasks.get();
    let follow = Follow::of(at, tasks.ready());
    let changes = match follow {
      Follow::RunChanges => feed.changes.get(),
      Follow::Integration | Follow::Never => 0,
    };
    let runs = self.coalesced(ctx, changes).max(self.runs_from(follow, changes));
    let jobs = match follow {
      Follow::RunChanges | Follow::Integration => self.jobs(feed.job_state.get()),
      Follow::Never => 0,
    };
    Wake { retry, runs, jobs }
  }

  /// The runner's count of changes when the page started following them (opened on, or switched to, a run an
  /// agent works in), while it follows them; the count then is no change, so the page reads once, not again when
  /// [`Self::coalesced`] catches up with it.
  fn runs_from(&self, follow: Follow, changes: u64) -> u64 {
    let mut from = self.runs_from.lock();
    match follow {
      Follow::RunChanges => *from.get_or_insert(changes),
      Follow::Integration | Follow::Never => {
        *from = None;
        0
      }
    }
  }

  /// The runner's count of changes `changes` as it wakes the reads: at once when they have not woken for
  /// [`COALESCE`], else once that much time has passed since, with the count then; nothing polls in between.
  fn coalesced(&self, ctx: &mut Ctx, changes: u64) -> u64 {
    let woke = self.runs_woke.clone();
    let state = ctx
      .future(changes, move |changes| {
        let woke = woke.clone();
        async move {
          let since = *woke.lock();
          let wait = since.map_or(Duration::ZERO, |at| {
            (at + COALESCE).saturating_duration_since(Instant::now())
          });
          tokio::time::sleep(wait).await;
          *woke.lock() = Some(Instant::now());
          Ok::<_, ()>(changes)
        }
      })
      .state()
      .get();
    state.data.unwrap_or(0)
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
