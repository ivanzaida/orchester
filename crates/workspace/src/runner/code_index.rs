//! *Rebuild* and *Retry* of a code index ([`Command::RebuildCodeIndex`]): the project's runner builds the index again
//! where it keeps the indexes its runs' agents read (`docs/architecture/remote-runners.md`); the desktop builds none.

use orchester_core::revision::Revision;
use orchester_ipc::{Command, Reply};

use super::{RequestError, RunRequestError, Runners};
use crate::workspace::OpenProject;

impl Runners {
  /// Asks `project`'s runner, starting it if needed, to build the index of `revision` again; answers once it is
  /// built.
  pub async fn rebuild_code_index_in(&self, project: &OpenProject, revision: Revision) -> Result<(), RunRequestError> {
    match self
      .ask_project(project, Command::RebuildCodeIndex { revision })
      .await?
    {
      Reply::CodeIndexRebuilt { .. } => Ok(()),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to RebuildCodeIndex");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }
}
