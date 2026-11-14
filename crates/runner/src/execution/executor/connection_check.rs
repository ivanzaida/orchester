//! *Test connection* and *Fetch models* for a native connection (`docs/architecture/native-agent.md` §7):
//! the runner reads the connection's API key from the keychain, lists the model API's models, which spends
//! no tokens, and answers with what that found and the models listed. The desktop never reads the key,
//! and the key reaches no reply, log line or error.

use std::sync::Arc;

use orchester_agent::{Endpoint, WireQuirks, check_anthropic};
use orchester_core::{
  connection::{AgentProvider, ApiEndpoint, CheckResult, NativeModel},
  id::{ApiAccountId, ProjectId, SecretId},
  secret::{SecretValue, VaultError},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use crate::{computer::ComputerResources, execution::Scheduler};

impl Scheduler {
  /// Tests `api`, a model API of `provider`, with its key: what the test found, and the models it
  /// listed as fetched now.
  pub async fn check_connection(
    &self,
    provider: AgentProvider,
    api: ApiEndpoint,
  ) -> Result<(CheckResult, Vec<NativeModel>), Rejection> {
    let failed = |detail: String| Rejection::Failed { detail };
    if !provider.is_native() || !provider.runs_agents() {
      return Err(failed(
        "this version cannot test a connection to this provider".to_owned(),
      ));
    }
    let db = ProjectDb::open(&self.inner.project_root).map_err(|error| failed(error.to_string()))?;
    let key = api_key(self.inner.computer.as_ref(), db.project().id(), api.key()).map_err(failed)?;
    let endpoint = Endpoint::new(api.base_url().clone(), key, WireQuirks::anthropic(&api))
      .with_provider_fallback(api.provider_fallback());
    let checked = check_anthropic(endpoint).await;
    if let Some(error) = &checked.error {
      tracing::info!(%error, url = api.base_url().as_str(), "the model API did not pass its test");
    }
    let at = Timestamp::now();
    let models = checked
      .models
      .iter()
      .map(|model| NativeModel::listed(model, at))
      .collect();
    Ok((checked.result, models))
  }
}

/// The API key `key` names, a native connection's or an API provider's, read through `computer`'s keychain: the
/// secret of `project` by that id, and, when the project has none, the key of the person's API account by that id
/// (a connection on an API account keeps the account's id as its key's: `docs/design/agent-accounts.md`, "Agents
/// run on accounts"). An archived secret, or a key the keychain does not give, is refused, named, never with its
/// value.
pub(in crate::execution) fn api_key(
  computer: &dyn ComputerResources,
  project: ProjectId,
  key: SecretId,
) -> Result<Arc<SecretValue>, String> {
  const MISSING: &str = "the connection's API key secret is missing or archived";
  let Some(secret) = computer.secret(key).map_err(|error| error.to_string())? else {
    let account = ApiAccountId::from_uuid(key.as_uuid());
    return computer
      .api_account_key(account)
      .map(Arc::new)
      .map_err(|error| match error {
        VaultError::NotFound => MISSING.to_owned(),
        error => {
          tracing::warn!(%account, %error, "the keychain did not give an API account's key");
          "the keychain did not give the key of the connection's API account".to_owned()
        }
      });
  };
  if secret.is_archived() {
    return Err(MISSING.to_owned());
  }
  computer.secret_value(project, &secret).map(Arc::new).map_err(|error| {
    tracing::warn!(secret = %secret.name(), %error, "the keychain did not give a connection's API key");
    format!("the keychain did not give the API key {}", secret.name())
  })
}
