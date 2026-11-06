//! Reading the attempt page (`merge_attempts::page`) off the UI thread, again whenever the project's tasks or runs
//! change, the merge queue moves, a merge job starts or ends, or a button of the page changed something (`again`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{project::TaskKey, run::RunNumber};
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::merge_attempts::page::{AttemptPage, attempt_page},
};

use crate::{project::ShownProject, services::Services};

/// What the page shows of the run.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Read {
  Reading,
  /// No such task or run in the project.
  Missing,
  Ready(Arc<AttemptPage>),
  Failed(Message),
}

/// Run `number` of the task `key` in `project`, as last read; the task page's Merge row reads it too.
pub(crate) fn read(ctx: &mut Ctx, project: &ShownProject, (key, number): (&TaskKey, RunNumber), again: u64) -> Read {
  let feed = &project.feed;
  let Some(tasks) = feed.tasks.with(|tasks| tasks.ready().cloned()) else {
    return Read::Reading;
  };
  let root = project.open.repo_root.clone();
  let queue = crate::project::merge_queue::changes(ctx, &root);
  let changes = (queue, feed.saves.get(), feed.states.get(), feed.job_state.get(), again);
  let services = Services::of(ctx);
  let (programs, hold) = (services.programs, services.hold);
  let loaded = ctx
    .future(
      (root, tasks, key.clone(), number, changes),
      move |(root, tasks, key, number, _)| {
        let (programs, hold) = (programs.clone(), hold.clone());
        async move {
          hold.released().await;
          background::run(move || attempt_page(&tasks, (&root, &programs), (&key, number)))
            .await?
            .map_err(|error| messages::store_error(&error))
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
    Some(None) => Read::Missing,
    Some(Some(page)) => Read::Ready(Arc::new(page)),
  }
}
