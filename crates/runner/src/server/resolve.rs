//! A person's resolving by hand in a run's worktree ([`Command::Resolve`](orchester_ipc::Command::Resolve)), answered
//! once git there is done.

use orchester_core::{id::RunId, integration::resolve::request::ResolveRequest};
use orchester_ipc::Reply;

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  /// Answers `request` about `run`'s worktree once it was carried out there.
  pub(super) fn resolve(&self, scheduler: Scheduler, run: RunId, request: ResolveRequest) -> Answer {
    self.reply(async move {
      match scheduler.resolve(run, request).await {
        Ok(answer) => Reply::Resolved(answer),
        Err(failure) => Reply::ResolveFailed(failure),
      }
    })
  }
}
