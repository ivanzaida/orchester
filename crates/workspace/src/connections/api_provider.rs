//! Agent CLI connections on API providers (`docs/design/api-providers.md`): adding or editing a Codex or Claude Code
//! connection that runs on a provider's API with its key secret picked from the project's secrets or created with it,
//! and changing or removing the provider of a stored connection. The key's value goes to the keychain only and is
//! never read back here. Every function blocks on SQLite or on the keychain; the UI and the tools call them off the UI
//! thread.

use std::path::Path;

use orchester_core::{
  connection::{ApiProvider, ApiProviderError, BaseUrl, Connection, ConnectionAccount, ConnectionError, EndpointError},
  id::{ConnectionId, SecretId},
  resource::ModelId,
  secret::{SecretValue, SecretVault},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::{ConnectionDraft, SaveError, save_with},
  native::{KeySecret, with_key},
  probe,
};
use crate::messages::Message;

/// An API provider as the connection form saves it: the key secret picked or to be created, the models typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderDraft {
  pub base_url: BaseUrl,
  pub key: KeySecret,
  pub models: Vec<ModelId>,
}

/// What an agent CLI connection runs on, as the form saves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunsOn {
  /// The CLI's own sign-in, on this account.
  Account(ConnectionAccount),
  Provider(ProviderDraft),
}

/// Adds or updates the CLI connection in `draft`, with its check, on what `runs_on` names. A new key secret is added
/// first with `value`, and archived again if the connection cannot be saved.
pub fn save_cli_in(
  repo_root: &Path,
  vault: &dyn SecretVault,
  draft: ConnectionDraft,
  runs_on: RunsOn,
  value: Option<SecretValue>,
) -> Result<ConnectionId, Message> {
  let mut db = ProjectDb::open(repo_root).map_err(|error| SaveError::from(error).message())?;
  let now = Timestamp::now();
  match runs_on {
    RunsOn::Account(account) => {
      save_with(&mut db, draft, (Some(account), Some(None)), now).map_err(|error| failure(&error))
    }
    RunsOn::Provider(provider) => with_key(&mut db, vault, (&provider.key, value), |db, key| {
      let api = provider_of(provider.base_url, key, provider.models)?;
      save_with(db, draft, (Some(ConnectionAccount::Default), Some(Some(api))), now).map_err(|error| failure(&error))
    }),
  }
}

/// Adds the CLI connection in `draft`, with its check, on `provider`, whose key secret is one of the project's; refused
/// as [`super::data::save`] refuses a CLI the check did not find.
pub fn save_on_provider_in(
  repo_root: &Path,
  draft: ConnectionDraft,
  provider: ApiProvider,
) -> Result<ConnectionId, SaveError> {
  if !draft.check.result.is_verified() {
    return Err(SaveError::NotVerified);
  }
  let mut db = ProjectDb::open(repo_root)?;
  save_with(
    &mut db,
    draft,
    (Some(ConnectionAccount::Default), Some(Some(provider))),
    Timestamp::now(),
  )
}

/// The API provider of `base_url`, the key in secret `key` and `models`, or why it cannot be one.
pub fn provider_of(base_url: BaseUrl, key: SecretId, models: Vec<ModelId>) -> Result<ApiProvider, Message> {
  ApiProvider::new(base_url, key, models).map_err(|error| match error {
    ApiProviderError::NoModels => Message::new("connections.provider.noModels"),
  })
}

/// The base URL typed as `value`, or what is wrong with it.
pub fn base_url(value: &str) -> Result<BaseUrl, Message> {
  BaseUrl::new(value).map_err(|error| {
    Message::new(match error {
      EndpointError::BlankUrl => "connections.provider.baseUrl.blank",
      EndpointError::UrlScheme => "connections.provider.baseUrl.scheme",
      EndpointError::NoHost | EndpointError::InvalidHost => "connections.provider.baseUrl.host",
      EndpointError::UrlCredentials => "connections.provider.baseUrl.credentials",
      EndpointError::UrlQuery => "connections.provider.baseUrl.query",
      EndpointError::Whitespace => "connections.provider.baseUrl.whitespace",
      EndpointError::Backslash => "connections.provider.baseUrl.backslash",
      EndpointError::TooLong => "connections.provider.baseUrl.tooLong",
    })
  })
}

/// The model ids typed as `value`, separated by commas, spaces or line breaks, or the first that is not one.
pub fn model_ids(value: &str) -> Result<Vec<ModelId>, Message> {
  value
    .split(|c: char| c == ',' || c.is_whitespace())
    .filter(|part| !part.is_empty())
    .map(model_id)
    .collect()
}

/// The model id `value`, or why it is not one.
pub fn model_id(value: &str) -> Result<ModelId, Message> {
  ModelId::new(value).map_err(|_| Message::new("connections.provider.modelInvalid").with("model", value.to_owned()))
}

/// Runs stored connection `id` of the project at `repo_root` on `provider`, or on its CLI's own sign-in again
/// (`None`), on the CLI's Default account. Leaving another account drops the check made there, so the CLI is checked
/// again on Default.
pub fn set_provider_in(repo_root: &Path, id: ConnectionId, provider: Option<ApiProvider>) -> Result<(), SaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let mut connection = db.connection(id)?.ok_or(StoreError::ConnectionNotFound(id))?;
  if provider.is_some() {
    connection.set_account(ConnectionAccount::Default)?;
  }
  connection.set_api_provider(provider)?;
  if connection.last_check().is_none()
    && let Some(check) = probe::recheck(&connection)
  {
    connection
      .record_check(check)
      .expect("the check was made for the connection's own kind");
  }
  db.save_connection(&connection)?;
  Ok(())
}

/// Runs stored connection `id` of the project at `repo_root` on what `runs_on` names, as Edit connection saves it: an
/// account of its CLI, or a provider whose new key secret is added first with `value` (and archived again if the
/// connection cannot be saved). A check made on another account is dropped, so the CLI is checked again.
pub fn set_runs_on_in(
  repo_root: &Path,
  vault: &dyn SecretVault,
  id: ConnectionId,
  runs_on: RunsOn,
  value: Option<SecretValue>,
) -> Result<(), Message> {
  let mut db = ProjectDb::open(repo_root).map_err(|error| SaveError::from(error).message())?;
  let stored = |db: &ProjectDb| -> Result<Connection, Message> {
    db.connection(id)
      .map_err(|error| SaveError::from(error).message())?
      .ok_or_else(|| SaveError::from(StoreError::ConnectionNotFound(id)).message())
  };
  match runs_on {
    RunsOn::Account(account) => {
      let mut connection = stored(&db)?;
      runs_on_and_saved(&mut db, &mut connection, (account, None))
    }
    RunsOn::Provider(provider) => with_key(&mut db, vault, (&provider.key, value), |db, key| {
      let api = provider_of(provider.base_url, key, provider.models)?;
      let mut connection = stored(db)?;
      runs_on_and_saved(db, &mut connection, (ConnectionAccount::Default, Some(api)))
    }),
  }
}

/// `connection` on `account` and `provider`, checked again when that dropped its check, and saved.
fn runs_on_and_saved(
  db: &mut ProjectDb,
  connection: &mut Connection,
  (account, provider): (ConnectionAccount, Option<ApiProvider>),
) -> Result<(), Message> {
  let refused = |error: ConnectionError| failure(&SaveError::from(error));
  connection.set_api_provider(None).map_err(refused)?;
  connection.set_account(account).map_err(refused)?;
  connection.set_api_provider(provider).map_err(refused)?;
  if connection.last_check().is_none()
    && let Some(check) = probe::recheck(connection)
  {
    connection
      .record_check(check)
      .expect("the check was made for the connection's own kind");
  }
  db.save_connection(connection)
    .map_err(|error| failure(&SaveError::from(error)))
}

fn failure(error: &SaveError) -> Message {
  tracing::error!(%error, "cannot save the connection");
  error.message()
}
