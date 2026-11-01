use orchester_core::connection::{AgentProvider, ApiEndpoint};
use orchester_ipc::Reply;

use super::{Answer, Later, Scheduler};

pub(super) fn answer(later: &Later<'_>, scheduler: Scheduler, provider: AgentProvider, api: ApiEndpoint) -> Answer {
  later.reply(async move {
    match scheduler.check_connection(provider, api).await {
      Ok((result, models)) => Reply::ConnectionChecked { result, models },
      Err(rejection) => Reply::Rejected(rejection),
    }
  })
}
