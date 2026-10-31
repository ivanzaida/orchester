//! The *Browser* window's requests ([`Command::Browser`](orchester_ipc::Command::Browser)), answered once the
//! project's browser carried them out.

use orchester_ipc::{Reply, browser::BrowserCommand};

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  /// Answers the window's `command` once the project's browser carried it out. A desktop that subscribes gets the
  /// browser's events with the replies.
  pub(super) fn browser(&self, scheduler: Scheduler, command: BrowserCommand) -> Answer {
    let desktop = self.replies.clone();
    // Here, as the request is read: the tasks that carry requests out start in no certain order.
    let arrived = scheduler.browser().arrived(command);
    self.reply(async move {
      match scheduler.browser().serve(arrived, &desktop).await {
        Ok(reply) => Reply::Browser(reply),
        Err(rejection) => Reply::Rejected(rejection),
      }
    })
  }
}
