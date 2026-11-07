//! Reading the task's review page (`review_threads::page`) off the UI thread, again whenever the project's tasks or
//! runs change state, a merge job ends, or a change of the page landed; and the run's review alone, again on every
//! change of the project's runs while a tab of it is active, so an answer streams in as Implement writes it.

use std::sync::Arc;

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::{id::RunId, project::TaskKey, revision::Revision};
use orchester_workspace::{
  background,
  messages::Message,
  tasks::{
    changes::Checkpointed,
    review_threads::{
      page::{ReviewPage, task_review_page},
      read::{Review, review_of},
      since::diff_since_review,
    },
  },
};

use super::state::Held;
use crate::{project::ShownProject, services::Services};

/// What the page shows of the run.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::screens::review) enum Read {
  Reading,
  /// The task has no run to review: no such task, or none of its runs waits for review or was sent back from it.
  Nothing,
  Ready(Arc<ReviewPage>),
  Failed(Message),
}

/// The review of the task `key` in `project` as last read; `again` counts what should read it again.
pub(in crate::screens::review) fn read(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey, again: u64) -> Read {
  let feed = &project.feed;
  let Some(tasks) = feed.tasks.with(|tasks| tasks.ready().cloned()) else {
    return Read::Reading;
  };
  let root = project.open.repo_root.clone();
  let changes = (feed.saves.get(), feed.states.get(), feed.job_state.get(), again);
  let services = Services::of(ctx);
  let (programs, hold) = (services.programs, services.hold);
  let loaded = ctx
    .future((root, tasks, key.clone(), changes), move |(root, tasks, key, _)| {
      let (programs, hold) = (programs.clone(), hold.clone());
      async move {
        hold.released().await;
        background::run(move || task_review_page(&tasks, (&root, &programs), &key))
          .await?
          .map(|page| Held(page.map(Arc::new)))
          .map_err(|refusal| refusal.message())
      }
    })
    .state()
    .get();
  if let Some(error) = loaded.error.clone().filter(|_| loaded.is_rejected()) {
    return Read::Failed(error);
  }
  match loaded.data {
    None => Read::Reading,
    Some(Held(None)) => Read::Nothing,
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

/// What the page reads of the commits after the head reviewed.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum SinceRead {
  Reading,
  Ready(Arc<Checkpointed>),
  /// Why there is no diff since the review ([`diff_since_review`]).
  Failed(Message),
}

/// The diff from `reviewed` to `head` as read once, with what it was read for.
#[derive(Clone, Debug, PartialEq)]
struct SinceDone {
  reviewed: Option<Revision>,
  head: Option<Revision>,
  outcome: Result<Arc<Checkpointed>, Message>,
}

impl DevtoolsInspectable for SinceDone {}

/// The diff since the review of the run's branch at `head`, read when `wanted`, again once the branch moves; `None`
/// while it is not wanted.
pub(super) fn since(
  ctx: &mut Ctx,
  project: &ShownProject,
  (reviewed, head): (Option<Revision>, Option<Revision>),
  wanted: bool,
) -> Option<SinceRead> {
  let wanted = wanted && head.is_some();
  let root = project.open.repo_root.clone();
  let programs = Services::of(ctx).programs;
  let loaded = ctx
    .future(
      (root, reviewed.clone(), head.clone(), wanted),
      move |(root, reviewed, head, wanted)| {
        let programs = programs.clone();
        async move {
          if !wanted {
            return Ok(Held(None));
          }
          background::run(move || {
            let outcome = diff_since_review((&root, &programs), (reviewed.as_ref(), head.as_ref()))
              .map(Arc::new)
              .map_err(|error| error.message());
            Held(Some(SinceDone {
              reviewed,
              head,
              outcome,
            }))
          })
          .await
        }
      },
    )
    .state()
    .get();
  if !wanted {
    return None;
  }
  let rejected = loaded.is_rejected();
  Some(match loaded.data {
    // A pending read keeps the result of the one before: only the one for this head counts.
    Some(Held(Some(done))) if done.reviewed == reviewed && done.head == head => match done.outcome {
      Ok(read) => SinceRead::Ready(read),
      Err(reason) => SinceRead::Failed(reason),
    },
    _ => match loaded.error.filter(|_| rejected) {
      Some(reason) => SinceRead::Failed(reason),
      None => SinceRead::Reading,
    },
  })
}
