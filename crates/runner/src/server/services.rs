//! The Services tab's requests ([`Command::Services`](orchester_ipc::Command::Services)), answered once carried out
//! without holding up the runner's other requests: a stop or a restart waits for the service's processes.

use orchester_ipc::{Reply, services::ServiceCommand};

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  pub(super) fn services(&self, scheduler: Scheduler, command: ServiceCommand) -> Answer {
    self.reply(async move {
      match scheduler.services(command).await {
        Ok(reply) => Reply::Services(reply),
        Err(rejection) => Reply::Rejected(rejection),
      }
    })
  }
}
