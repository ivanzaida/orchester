//! What the chat's runs-on picker shows of the connections the Orchestrator can run on, besides what its settings list
//! ([`super::settings::ConnectionChoice`]): whose sign-in a connection's CLI runs on, or the API provider it runs on
//! with the name of its key's secret and the tokens the project's agents spent on it from a time on.

use std::path::Path;

use orchester_core::{
  connection::{BaseUrl, ConnectionAccount},
  id::{ConnectionId, SecretId},
  store::SecretStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::messages::{self, Message};

/// What a connection's CLI runs on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunsOn {
  /// Its own sign-in, on this account.
  SignIn(ConnectionAccount),
  /// An API provider at `base_url`, with its key in secret `key`.
  ApiProvider { base_url: BaseUrl, key: SecretId },
  /// A native connection's model API.
  Native,
}

/// An API provider's key secret by its name (`None` when the secret is gone), and the tokens spent on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderUse {
  pub connection: ConnectionId,
  pub key: Option<String>,
  pub tokens: u64,
}

/// What the project at `root` spent from `since` on on each of `providers` (a connection with its provider's base URL
/// and key secret), and their key secrets' names. Blocks on SQLite.
pub fn providers_use_in(
  root: &Path,
  providers: &[(ConnectionId, BaseUrl, SecretId)],
  since: Timestamp,
) -> Result<Vec<ProviderUse>, Message> {
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  providers
    .iter()
    .map(|(connection, base_url, key)| {
      let name = db
        .secret(*key)
        .map_err(|error| messages::store_error(&error))?
        .filter(|secret| !secret.is_archived())
        .map(|secret| secret.name().as_str().to_owned());
      let tokens = db
        .api_provider_tokens_since((base_url, *key), since)
        .map_err(|error| messages::store_error(&error))?;
      Ok(ProviderUse {
        connection: *connection,
        key: name,
        tokens,
      })
    })
    .collect()
}
