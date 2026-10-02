//! Native agent connections as the Connections screen sets them up (`docs/architecture/native-agent.md`
//! §7): adding or editing one, with its key secret picked from the project's secrets or created with it,
//! and saving the models added by hand. The key's value goes to the keychain only and is never read
//! back here. Every function blocks on SQLite or on the keychain; the UI calls them off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, ConnectionError, EndpointPreset, NativeModel, NativeModels,
    NativeModelsError, ensure_used_listed,
  },
  id::{ConnectionId, SecretId},
  name::Name,
  resource::ModelId,
  secret::{self, NewSecret, SecretValue, SecretVault},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::data::ProfileUse;
use crate::{
  agents::data::profiles,
  messages::{self, Message},
};

/// A profile on a native connection and the model it runs with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelUse {
  pub role: Arc<str>,
  pub model: ModelId,
}

/// The profiles among `uses` that run on `connection`, with their models.
pub fn model_uses(uses: &[ProfileUse], connection: ConnectionId) -> Vec<ModelUse> {
  uses
    .iter()
    .filter(|used| used.connection == connection)
    .map(|used| ModelUse {
      role: used.role.clone(),
      model: used.model.clone(),
    })
    .collect()
}

/// Refuses `models` while a profile in `uses` runs on a model they no longer list (core's
/// [`ensure_used_listed`]); `key` says so, naming the first such profile and its model.
pub fn keep_used(models: &[NativeModel], uses: &[ModelUse], key: &'static str) -> Result<(), Message> {
  let Err(NativeModelsError::UsedModelDropped(model)) = ensure_used_listed(models, uses.iter().map(|used| &used.model))
  else {
    return Ok(());
  };
  let profile = uses
    .iter()
    .find(|used| used.model == model)
    .map(|used| used.role.to_string())
    .unwrap_or_default();
  Err(
    Message::new(key)
      .with("profile", profile)
      .with("model", model.as_str().to_owned()),
  )
}

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
/// `vault`; if the connection cannot be stored then, that secret is archived: its value is removed
/// from the keychain and it leaves the project's secrets, while its record stays in the project
/// database as archived.
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
    // The connection was not stored, so nothing refers to the secret just added; archiving removes its
    // value but keeps its record.
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
  ensure_kept(db, id, &draft.models)?;
  let mut connection = stored(db, id)?;
  connection.rename(draft.name.clone());
  connection.set_api(api).map_err(|error| connection_failure(&error))?;
  db.save_connection(&connection).map_err(|error| store_failure(&error))?;
  Ok(id)
}

/// Stores `models` as the models of native connection `id`; its key, endpoint and last check stay.
pub fn save_models_in(repo_root: &Path, id: ConnectionId, models: NativeModels) -> Result<(), Message> {
  let mut db = ProjectDb::open(repo_root).map_err(|error| store_failure(&error))?;
  ensure_kept(&db, id, &models)?;
  let mut connection = stored(&db, id)?;
  let api = connection
    .api()
    .ok_or_else(|| Message::new("errors.storage"))?
    .clone()
    .with_models(models);
  connection.set_api(api).map_err(|error| connection_failure(&error))?;
  db.save_connection(&connection).map_err(|error| store_failure(&error))
}

/// Refuses `models` for connection `id` while a profile that runs on it, as stored now, uses a model
/// they no longer list.
fn ensure_kept(db: &ProjectDb, id: ConnectionId, models: &NativeModels) -> Result<(), Message> {
  let uses: Vec<ModelUse> = profiles(db)
    .map_err(|error| store_failure(&error))?
    .into_iter()
    .filter(|entry| entry.profile.connection() == id)
    .map(|entry| ModelUse {
      role: entry.profile.role().as_str().into(),
      model: entry.profile.model().clone(),
    })
    .collect();
  keep_used(models.models(), &uses, "connections.models.usedDropped").inspect_err(|_| {
    tracing::warn!(connection = %id, "a profile still uses a model the saved models leave out");
  })
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
