//! *Fix CI automatically* (ORC-335, `orchester_workspace::pull_requests::ci_fix`): while the project is open and its
//! runner is online, when it opens and every [`POLL_INTERVAL`] after, the CI of each run whose switch is on is read
//! (answered by the app's reads of the last 30 s), and a failure no Fix CI answered yet goes to the run's agent on its
//! own, up to the run's limit; at the limit the run records that it stopped. Whatever it sent or stopped at reads the
//! project's tasks again. Mounted with the project's feed, so it follows the project whether or not a task's page is
//! open. Renders nothing.

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_core::{
  pull_request::ci::{POLL_INTERVAL, fix::failure_of},
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::Message,
  pull_requests::{
    ci::PullRequestCi,
    ci_fix::{FixDone, Fixer, auto_fix_runs_in, fix_ci},
  },
  workspace::OpenProject,
};

use super::ProjectFeed;
use crate::services::Services;

/// Sends `open`'s automatic Fix CI that are due, while its runner is `online`.
pub(super) fn follow(ctx: &mut Ctx, (open, feed): (&OpenProject, &ProjectFeed), online: bool) {
  let services = Services::of(ctx);
  let (open, feed) = (open.clone(), feed.clone());
  let _following = ctx.stream(
    (open.repo_root.clone(), online),
    move |(_, online), emitter: StreamEmitter<u64, ()>| {
      let (services, open, feed) = (services.clone(), open.clone(), feed.clone());
      async move {
        // Without its runner no message of a run goes anywhere: nothing is sent until it is back.
        if !online {
          return;
        }
        let mut done = 0;
        loop {
          match fix_due(&services, &open).await {
            Ok(0) => {}
            Ok(count) => {
              done += count;
              feed.saved();
            }
            Err(error) => tracing::warn!(?error, "cannot send the project's automatic Fix CI"),
          }
          if !emitter.emit(done) {
            return;
          }
          tokio::time::sleep(POLL_INTERVAL).await;
        }
      }
    },
  );
}

/// Sends the automatic Fix CI of each run of `open` whose switch is on and whose CI failed; how many it sent or stopped
/// at.
async fn fix_due(services: &Services, open: &OpenProject) -> Result<u64, Message> {
  let root = open.repo_root.clone();
  let runs = background::run(move || auto_fix_runs_in(&root))
    .await?
    .map_err(|error| error.message())?;
  let mut done = 0;
  for due in runs {
    let (reads, programs, root) = (
      services.ci_reads.clone(),
      services.programs.clone(),
      open.repo_root.clone(),
    );
    let project = open.project.id();
    let ci = background::run(move || reads.read((project, &root), &programs, due.number, Timestamp::now()))
      .await?
      .map_err(|error| error.message())?;
    let PullRequestCi::Read(read) = ci else {
      continue;
    };
    if failure_of(&read.checks).is_none() {
      continue;
    }
    let runner = (&services.runners, &services.servers);
    let ready = (&services.workspace, &services.vault);
    let (fixed, _) = fix_ci(
      runner,
      ready,
      open,
      (due.task, due.run),
      (read, false),
      Fixer::Automatic,
    )
    .await?;
    if matches!(fixed, FixDone::Stored(_) | FixDone::Stopped) {
      done += 1;
    }
  }
  Ok(done)
}
