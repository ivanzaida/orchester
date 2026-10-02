//! Native agent connections as the Connections screen sets them up (`docs/architecture/native-agent.md`
//! §7): adding or editing one, with its key secret picked from the project's secrets or created with it,
//! and saving the models added by hand. The key's value goes to the keychain only and is never read
//! back here. Every function blocks on SQLite or on the keychain; the UI calls them off the UI thread.

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, ConnectionError, EndpointPreset, NativeModels},
  id::{ConnectionId, SecretId},
  name::Name,
  secret::{self, NewSecret, SecretValue, SecretVault},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::{self, Message};

/// The project secret holding the key: one the project has, or one created with the connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeySecret {
  Existing(SecretId),
  New(NewSecret),
}

/// A native connection as the dialog saves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeDraft {
  /// The connection being edited; `None` adds a new one.
  pub id: Option<ConnectionId>,
  pub name: Name,
  pub provider: AgentProvider,
  pub preset: EndpointPreset,
  pub base_url: BaseUrl,
  pub key: KeySecret,
  pub models: NativeModels,
  pub provider_fallback: bool,
}

impl lurq::app::component::DevtoolsInspectable for NativeDraft {}

/// Adds or updates the connection in `draft`. A new key secret is added first, its `value` into
/// `vault`; if the connection cannot be stored then, the secret is archived again, so a failed save
/// leaves no secret behind.
pub fn save_native_in(
  repo_root: &Path,
  vault: &dyn SecretVault,
  draft: &NativeDraft,
  value: Option<SecretValue>,
) -> Result<ConnectionId, Message> {
  let mut db = ProjectDb::open(repo_root).map_err(|error| store_failure(&error))?;
  let project = db.project().id();
  let now = Timestamp::now();
  let (key, created) = match &draft.key {
    KeySecret::Existing(id) => (*id, false),
    KeySecret::New(new) => {
      let value = value.ok_or_else(|| Message::new("connections.native.key.valueRequired"))?;
      let added = secret::add(&mut db, vault, project, new.clone(), &value, now).map_err(|error| {
        tracing::error!(%error, "cannot add the connection's key secret");
        messages::secret_error(&error)
      })?;
      (added.id(), true)
    }
  };
  let saved = store_connection(&mut db, draft, key, now);
  if saved.is_err() && created {
    // The connection was not stored, so nothing refers to the secret just added.
    if let Err(error) = secret::archive(&mut db, vault, project, key, now) {
      tracing::error!(%error, "cannot archive the key secret of a connection that was not saved");
    }
  }
  saved
}

fn store_connection(
  db: &mut ProjectDb,
  draft: &NativeDraft,
  key: SecretId,
  now: Timestamp,
) -> Result<ConnectionId, Message> {
  let api = ApiEndpoint::new(draft.preset, draft.base_url.clone(), key)
    .with_models(draft.models.clone())
    .with_provider_fallback(draft.provider_fallback);
  let Some(id) = draft.id else {
    let connection = AgentConnection::native(ConnectionId::generate(), draft.name.clone(), draft.provider, api, now)
      .map_err(|error| connection_failure(&error))?;
    db.create_connection(&connection)
      .map_err(|error| store_failure(&error))?;
    return Ok(connection.id());
  };
  let mut connection = stored(db, id)?;
  connection.rename(draft.name.clone());
  connection.set_api(api).map_err(|error| connection_failure(&error))?;
  db.save_connection(&connection).map_err(|error| store_failure(&error))?;
  Ok(id)
}

/// Stores `models` as the models of native connection `id`; its key, endpoint and last check stay.
pub fn save_models_in(repo_root: &Path, id: ConnectionId, models: NativeModels) -> Result<(), Message> {
  let mut db = ProjectDb::open(repo_root).map_err(|error| store_failure(&error))?;
  let mut connection = stored(&db, id)?;
  let api = connection
    .api()
    .ok_or_else(|| Message::new("errors.storage"))?
    .clone()
    .with_models(models);
  connection.set_api(api).map_err(|error| connection_failure(&error))?;
  db.save_connection(&connection).map_err(|error| store_failure(&error))
}

fn stored(db: &ProjectDb, id: ConnectionId) -> Result<AgentConnection, Message> {
  let connection = db
    .connection(id)
    .map_err(|error| store_failure(&error))?
    .ok_or_else(|| store_failure(&StoreError::ConnectionNotFound(id)))?;
  AgentConnection::try_from(connection).map_err(|error| connection_failure(&error))
}

fn store_failure(error: &StoreError) -> Message {
  tracing::error!(%error, "cannot save the native connection");
  messages::store_error(error)
}

fn connection_failure(error: &ConnectionError) -> Message {
  tracing::error!(%error, "the native connection does not fit its provider");
  match error {
    ConnectionError::PresetNotOffered => Message::new("connections.native.preset.notOffered"),
    ConnectionError::FallbackNotOffered => Message::new("connections.native.fallback.notOffered"),
    // The dialog sets up native connections only; anything else means the stored data does not
    // match what was shown.
    ConnectionError::RelativeExecutable(_)
    | ConnectionError::NotAgent(_)
    | ConnectionError::NotGitHub(_)
    | ConnectionError::CheckKindMismatch(_)
    | ConnectionError::EndpointMismatch(_) => Message::new("errors.storage"),
  }
}
