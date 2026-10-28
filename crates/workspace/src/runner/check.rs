//! *Test connection* and *Fetch models* of a native connection, which its project's runner carries out
//! (`docs/architecture/native-agent.md` §7): it reads the API key, which the desktop never does, and lists the
//! model API's models.

use std::path::Path;

use orchester_core::connection::{AgentProvider, ApiEndpoint, CheckResult, NativeModel};
use orchester_ipc::{Command, Reply};

use super::{RequestError, RunRequestError, Runners};

impl Runners {
  /// Tests `api`, a model API of `provider`, through the runner of the project at `root`, whose key secret it
  /// names: what the test found, and the models it listed.
  pub async fn check_connection(
    &self,
    root: &Path,
    provider: AgentProvider,
    api: ApiEndpoint,
  ) -> Result<(CheckResult, Vec<NativeModel>), RunRequestError> {
    match self.ask(Some(root), Command::CheckConnection { provider, api }).await? {
      Reply::ConnectionChecked { result, models } => Ok((result, models)),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to CheckConnection");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }
}
