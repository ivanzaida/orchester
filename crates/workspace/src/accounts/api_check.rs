//! Testing an API account from Settings, with no project open (`docs/design/agent-accounts.md`, "Agents run on
//! accounts"): a model API Orchester's own agent calls is asked for its models with the account's key, as a native
//! connection's *Test connection* asks through a project's runner. Listing models spends no tokens. An agent CLI's
//! API provider is not tested here: a compatible server need not list models, and its CLI is what talks to it.
//!
//! The key is read from the keychain for the request and reaches no log, error or reply.

use std::sync::Arc;

use orchester_agent::{Endpoint, WireQuirks, check_anthropic, check_responses};
use orchester_core::{
  account::{ApiAccount, ApiSetup},
  connection::{AgentProvider, ApiEndpoint, ConnectionCheck},
  id::ApiAccountId,
  secret::{SecretVault, VaultKey},
  time::Timestamp,
};

use super::{AccountsError, api_runs_on::key_of};
use crate::workspace::Workspace;

impl Workspace {
  /// Asks the model API of the API account `id` for its models with the account's key and keeps what that found
  /// as the account's check. Refused for an agent CLI's provider ([`AccountsError::NotTested`]) and when the
  /// keychain does not give the key.
  pub async fn check_api_account(
    &self,
    vault: &dyn SecretVault,
    id: ApiAccountId,
  ) -> Result<ApiAccount, AccountsError> {
    let tested = self.listed_api_account(id)?;
    let ApiSetup::Native {
      provider,
      preset,
      base_url,
      provider_fallback,
      ..
    } = tested.setup()
    else {
      return Err(AccountsError::NotTested);
    };
    let key = vault
      .read(&VaultKey::api_account(id))
      .map_err(AccountsError::Keychain)?;
    let api = ApiEndpoint::new(*preset, base_url.clone(), key_of(id));
    let endpoint = Endpoint::new(base_url.clone(), Arc::new(key), WireQuirks::anthropic(&api))
      .with_provider_fallback(*provider_fallback);
    let checked = match provider {
      AgentProvider::AnthropicApi => check_anthropic(endpoint).await,
      AgentProvider::OpenAiApi => check_responses(endpoint).await,
      AgentProvider::ClaudeCodeCli | AgentProvider::CodexCli => return Err(AccountsError::NotTested),
    };
    if let Some(error) = &checked.error {
      tracing::info!(%error, url = base_url.as_str(), "the model API of an API account did not pass its test");
    }
    // Read again: the person may have renamed it, or pointed it elsewhere, while the API answered.
    let mut account = self.listed_api_account(id)?;
    if !account.setup().same_api(tested.setup()) {
      return Ok(account);
    }
    account
      .record_check(ConnectionCheck {
        at: Timestamp::now(),
        result: checked.result,
      })
      .map_err(AccountsError::ApiAccount)?;
    self.db.lock().save_api_account(&account)?;
    self.accounts_changed();
    Ok(account)
  }
}
