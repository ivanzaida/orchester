//! Reading the review page (`review_threads::page`) off the UI thread, again whenever the project's tasks or runs
//! change state, a merge job ends, or a change of the page landed; and the run's review alone, again on every change
//! of the project's runs while a tab of it is active, so an answer streams in as Implement writes it.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{id::RunId, project::TaskKey, run::RunNumber};
use orchester_workspace::{
  background,
  messages::Message,
  tasks::review_threads::{
    page::{ReviewPage, review_page},
    read::{Review, review_of},
  },
};

use super::state::Held;
use crate::{project::ShownProject, services::Services};

/// What the page shows of the run.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::screens::merges) enum Read {
  Reading,
  /// The run is not reviewed here: no such run, or its attempts tell it.
  Elsewhere,
  Ready(Arc<ReviewPage>),
  Failed(Message),
}

/// Run `number` of the task `key` in `project`, as last read; `again` counts what should read it again.
pub(in crate::screens::merges) fn read(
  ctx: &mut Ctx,
  project: &ShownProject,
  (key, number): (&TaskKey, RunNumber),
  again: u64,
) -> Read {
  let feed = &project.feed;
  let Some(tasks) = feed.tasks.with(|tasks| tasks.ready().cloned()) else {
    return Read::Reading;
  };
  let root = project.open.repo_root.clone();
  let changes = (feed.saves.get(), feed.states.get(), feed.job_state.get(), again);
  let services = Services::of(ctx);
  let (programs, hold) = (services.programs, services.hold);
  let loaded = ctx
    .future(
      (root, tasks, key.clone(), number, changes),
      move |(root, tasks, key, number, _)| {
        let (programs, hold) = (programs.clone(), hold.clone());
        async move {
          hold.released().await;
          background::run(move || review_page(&tasks, (&root, &programs), (&key, number)))
            .await?
            .map(|page| Held(page.map(Arc::new)))
            .map_err(|refusal| refusal.message())
        }
      },
    )
    .state()
    .get();
  if let Some(error) = loaded.error.clone().filter(|_| loaded.is_rejected()) {
    return Read::Failed(error);
  }
  match loaded.data {
    None => Read::Reading,
    Some(Held(None)) => Read::Elsewhere,
    Some(Held(Some(page))) => Read::Ready(page),
  }
}

/// `run`'s review as last read, read again on every change of the project's runs and of the page (`written`); `None`
/// until read, and when it cannot be read (the page's own read says why).
pub(super) fn live(ctx: &mut Ctx, project: &ShownProject, run: RunId, written: u64) -> Option<Arc<Review>> {
  let feed = &project.feed;
  let root = project.open.repo_root.clone();
  let changes = (feed.changes.get(), feed.states.get(), feed.saves.get(), written);
  let programs = Services::of(ctx).programs;
  let loaded = ctx
    .future((root, run, changes), move |(root, run, _)| {
      let programs = programs.clone();
      async move {
        background::run(move || review_of(&root, &programs, run))
          .await?
          .map(|review| Held(Arc::new(review)))
          .map_err(|refusal| {
            tracing::warn!(code = refusal.code(), %run, "cannot read the run's review again");
            refusal.message()
          })
      }
    })
    .state()
    .get();
  loaded.data.map(|Held(review)| review)
}
