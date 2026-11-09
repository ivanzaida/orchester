//! The task page's Services tab's side of a project's runner (ORC-265): its requests about a run's services
//! ([`Command::Services`]). A run's services live in its runner, so the requests go to the runner that is connected
//! and start none.

use std::path::Path;

use orchester_ipc::{
  Command, Reply,
  services::{ServiceCommand, ServiceReply},
};

use super::{RequestError, RunRequestError, Runners};

impl Runners {
  /// Sends the Services tab's `command` to the runner of the project at `root`; the answer once it was carried out.
  /// The command goes out at this call, before the future is awaited.
  pub fn services(
    &self,
    root: &Path,
    command: ServiceCommand,
  ) -> impl Future<Output = Result<ServiceReply, RunRequestError>> + Send + use<> {
    let answer = self.request(Some(root), Command::Services(command));
    async move {
      match answer?.await.unwrap_or(Err(RequestError::Lost))? {
        Reply::Services(reply) => Ok(reply),
        Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
        other => {
          tracing::error!(?other, "unexpected reply to a Services request");
          Err(RunRequestError::Request(RequestError::Lost))
        }
      }
    }
  }
}
