//! What the list shows of the Review list's read: off the UI thread, again whenever the project's tasks or runs change,
//! a merge job ends, the merge queue changes or the page's Try again was pressed. The list last read, kept in the
//! project's feed, stands in while the first read of a mount is under way.

use lurq::app::ctx::Ctx;
use orchester_core::time::Timestamp;
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::review_list::{ReviewList, review_list, waiting_count},
};

use crate::{
  project::{ShownProject, TasksState},
  services::Services,
};

/// What the page shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Read {
  /// The project's tasks are not read yet, so not even how many wait is known.
  Unknown,
  /// The list is read; `waiting` is how many *Waiting for you* will hold.
  Reading { waiting: usize },
  /// The list as read, or as last read while it is read anew.
  Ready(ReviewList),
  /// The project's tasks cannot be read.
  TasksFailed(Message),
  /// The list cannot be read, and why.
  Failed(Message),
}

/// The list as last read; `again` counts Try again.
pub(super) fn read(ctx: &mut Ctx, project: &ShownProject, again: u64) -> Read {
  let feed = &project.feed;
  let kept = || feed.review.get_untracked();
  let tasks = match feed.tasks.get() {
    TasksState::Ready(tasks) => tasks,
    TasksState::Loading => return kept().map_or(Read::Unknown, Read::Ready),
    TasksState::Failed(reason) => return Read::TasksFailed(reason),
  };
  let waiting = waiting_count(&tasks);
  let root = project.open.repo_root.clone();
  let queue = crate::project::merge_queue::changes(ctx, &root);
  let changes = (queue, feed.saves.get(), feed.states.get(), feed.job_state.get(), again);
  let services = Services::of(ctx);
  let (programs, hold) = (services.programs, services.hold);
  let loaded = ctx
    .future((root, tasks, changes), move |(root, tasks, _)| {
      let (programs, hold) = (programs.clone(), hold.clone());
      async move {
        hold.released().await;
        background::run(move || review_list(&tasks, (&root, &programs), Timestamp::now()))
          .await?
          .map_err(|error| messages::store_error(&error))
      }
    })
    .state()
    .get();
  if let Some(error) = loaded.error.clone().filter(|_| loaded.is_rejected()) {
    return Read::Failed(error);
  }
  if let Some(list) = loaded.fulfilled_data()
    && feed
      .review
      .with_untracked(|last| last.as_ref().is_none_or(|last| last != list))
  {
    feed.review.set(Some(list.clone()));
  }
  loaded.data.or_else(kept).map_or(Read::Reading { waiting }, Read::Ready)
}

/// The list as last read, for a page that needs only to know which tasks wait for the person (a task's review pages
/// through them): `None` until the first read is done. It is read as the list is, so the page follows what changes it.
pub(in crate::screens::review) fn last_read(ctx: &mut Ctx, project: &ShownProject) -> Option<ReviewList> {
  match read(ctx, project, 0) {
    Read::Ready(list) => Some(list),
    _ => None,
  }
}
