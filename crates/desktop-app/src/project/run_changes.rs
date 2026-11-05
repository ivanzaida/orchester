//! The change count of one run of a project, for a screen that shows that run alone: its runner's reports of that
//! run, its state and its transcript's lines among them, and of no run in particular (`Runners::run_changes_of`), but
//! nothing of the project's other runs. Like the feed's count of the project's changes ([`super::feed`]) it moves at
//! most once per [`TRANSCRIPT_PACE`], and only while a tab of the project is active
//! ([`super::ProjectFeed::following`]).

use std::path::PathBuf;

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_core::id::RunId;
use orchester_workspace::runner::Runners;

use super::{ShownProject, feed::TRANSCRIPT_PACE};
use crate::services::Services;

/// How often `run` of `project` changed while it is followed; `0` while `run` is `None` or no tab of the
/// project is active. Reading it in a render re-renders it when the count moves. Asked for in every render, whatever
/// `run` is: lurq keeps a component's streams by the order it asks for them.
pub fn of_run(ctx: &mut Ctx, project: &ShownProject, run: Option<RunId>) -> u64 {
  let runners = Services::of(ctx).runners;
  let followed = run.filter(|_| project.feed.following.get());
  let deps = (runners, project.open.repo_root.clone(), followed);
  let count = ctx.stream(
    deps,
    |(runners, root, run): (Runners, PathBuf, Option<RunId>), emitter: StreamEmitter<u64, ()>| async move {
      let Some(run) = run else {
        return;
      };
      let mut updates = runners.run_changes_of(&root, run);
      loop {
        let revision = *updates.borrow_and_update();
        if !emitter.emit(revision) {
          return;
        }
        tokio::time::sleep(TRANSCRIPT_PACE).await;
        if updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  count.state().get().data.unwrap_or(0)
}

/// What a view of `run` follows: the project's runs' changes of state (`ProjectFeed::states`), and while `live` (the
/// run goes on) the run's own changes, its transcript's lines too ([`of_run`]); never the lines of another run, which
/// only the project-wide `ProjectFeed::changes` counts.
pub fn of_shown_run(ctx: &mut Ctx, project: &ShownProject, run: Option<RunId>, live: bool) -> u64 {
  let own = of_run(ctx, project, run.filter(|_| live));
  project.feed.states.get() + if live { own } else { 0 }
}
