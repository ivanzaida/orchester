//! Native agent connections as the Connections screen sets them up (`docs/architecture/native-agent.md`
//! §7): adding or editing one, with its key secret picked from the project's secrets or created with it,
//! saving the models added by hand, and recording what a test through the runner found with the models it
//! listed. The key's value goes to the keychain only and is never read back here. Every function blocks on SQLite or on
//! the keychain; the UI calls them off the UI thread.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, CheckResult, ConnectionCheck, ConnectionError,
    EndpointPreset, NativeModel, NativeModels, with_fetched,
  },
  id::{ConnectionId, SecretId},
  name::Name,
  secret::{self, NewSecret, SecretValue, SecretVault},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::model_uses::{self, UsedKeys, keep_used};
use crate::{
  background,
  messages::{self, Message},
  runner::{RequestError, RunRequestError, Runners},
};

/// What a save refused for a model something still uses says.
pub const DROPPED_KEYS: UsedKeys = UsedKeys {
  profile: "connections.models.usedDropped",
  step: "connections.models.usedDroppedByStep",
};

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
  let now = Timestamp::now();
  with_key(&mut db, vault, (&draft.key, value), |db, key| {
    store_connection(db, draft, key, now)
  })
}

/// What `store` does with the key secret `key` names: one the project has, or one added first with `value` into
/// `vault`. If `store` fails then, the secret just added is archived: its value is removed from the keychain and it
/// leaves the project's secrets, while its record stays in the project database as archived.
pub(super) fn with_key<T>(
  db: &mut ProjectDb,
  vault: &dyn SecretVault,
  (key, value): (&KeySecret, Option<SecretValue>),
  store: impl FnOnce(&mut ProjectDb, SecretId) -> Result<T, Message>,
) -> Result<T, Message> {
  let project = db.project().id();
  let now = Timestamp::now();
  let (key, created) = match key {
    KeySecret::Existing(id) => (*id, false),
    KeySecret::New(new) => {
      let value = value.ok_or_else(|| Message::new("connections.native.key.valueRequired"))?;
      let added = secret::add(db, vault, project, new.clone(), &value, now).map_err(|error| {
        tracing::error!(%error, "cannot add the connection's key secret");
        messages::secret_error(&error)
      })?;
      (added.id(), true)
    }
  };
  let saved = store(db, key);
  if saved.is_err() && created {
    // The connection was not stored, so nothing refers to the secret just added; archiving removes its
    // value but keeps its record.
    if let Err(error) = secret::archive(db, vault, project, key, now) {
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
  ensure_kept(db, &connection, &draft.models)?;
  connection.rename(draft.name.clone());
  connection.set_api(api).map_err(|error| connection_failure(&error))?;
  db.save_connection(&connection).map_err(|error| store_failure(&error))?;
  Ok(id)
}

/// Stores `models` as the models of native connection `id`; its key, endpoint and last check stay.
pub fn save_models_in(repo_root: &Path, id: ConnectionId, models: NativeModels) -> Result<(), Message> {
  let mut db = ProjectDb::open(repo_root).map_err(|error| store_failure(&error))?;
  let mut connection = stored(&db, id)?;
  ensure_kept(&db, &connection, &models)?;
  let api = connection
    .api()
    .ok_or_else(|| Message::new("errors.storage"))?
    .clone()
    .with_models(models);
  connection.set_api(api).map_err(|error| connection_failure(&error))?;
  db.save_connection(&connection).map_err(|error| store_failure(&error))
}

/// *Test connection* and *Fetch models* of native connection `id` (`docs/architecture/native-agent.md` §7), in the
/// window and over MCP: the runner of the project at `repo_root` reads its key and lists the model API's models,
/// which spends no tokens, and what it found is recorded with the models it listed ([`record_tested_in`]). The
/// desktop never reads the key.
pub async fn test_in(runners: &Runners, repo_root: PathBuf, id: ConnectionId) -> Result<(), Message> {
  let root = repo_root.clone();
  let connection = background::run(move || load_in(&root, id)).await??;
  let api = connection.api().ok_or_else(|| Message::new("errors.storage"))?.clone();
  let (result, models) = runners
    .check_connection(&repo_root, connection.provider(), api.clone())
    .await
    .map_err(|error| test_failure(&error))?;
  background::run(move || record_tested_in(&repo_root, id, &api, result, models)).await?
}

/// Why a test through the runner did not happen.
fn test_failure(error: &RunRequestError) -> Message {
  tracing::warn!(%error, "the runner did not test the native connection");
  match error {
    RunRequestError::Request(RequestError::NotConnected) => Message::new("connections.native.testNoRunner"),
    RunRequestError::Request(RequestError::Lost) | RunRequestError::Rejected(_) => {
      Message::new("connections.native.testFailed")
    }
  }
}

/// [`record_check_in`] for a test of `tested`, native connection `id`'s endpoint when the test began. A test is
/// about one key and one endpoint: when a save changed either while it ran, what it found is not the stored
/// connection's, so nothing is recorded and the person is asked to test again.
pub fn record_tested_in(
  repo_root: &Path,
  id: ConnectionId,
  tested: &ApiEndpoint,
  result: CheckResult,
  fetched: Vec<NativeModel>,
) -> Result<(), Message> {
  let now = load_in(repo_root, id)?;
  let same = now.api().is_some_and(|api| api.same_api(tested));
  if !same {
    tracing::info!(connection = %id, "the connection's endpoint changed while it was tested");
    return Err(Message::new("connections.native.testStale"));
  }
  record_check_in(repo_root, id, result, fetched)
}

/// Records a test of native connection `id` (`docs/architecture/native-agent.md` §7): what it found, and
/// the models the API listed merged into its models.
pub fn record_check_in(
  repo_root: &Path,
  id: ConnectionId,
  result: CheckResult,
  fetched: Vec<NativeModel>,
) -> Result<(), Message> {
  let mut db = ProjectDb::open(repo_root).map_err(|error| store_failure(&error))?;
  let mut connection = stored(&db, id)?;
  if !fetched.is_empty() {
    let api = connection.api().ok_or_else(|| Message::new("errors.storage"))?.clone();
    let models = merged_models(api.models(), fetched)?;
    connection
      .set_api(api.with_models(models))
      .map_err(|error| connection_failure(&error))?;
  }
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result,
    })
    .map_err(|error| connection_failure(&error))?;
  db.save_connection(&connection).map_err(|error| store_failure(&error))
}

/// `models` with the models an endpoint listed, `fetched`, merged in (core's `with_fetched`); the default
/// model and effort stay while the models still take them. A default the merged models no longer take is left out,
/// and logged: the connection then runs on the model's own effort until a person chooses another.
pub fn merged_models(models: &NativeModels, fetched: Vec<NativeModel>) -> Result<NativeModels, Message> {
  let merged = with_fetched(models.models(), fetched);
  let default = models.default_model().cloned();
  NativeModels::new(merged.clone(), default.clone(), models.default_effort())
    .or_else(|_| {
      tracing::info!("the fetched models no longer take the default effort; it is left out");
      NativeModels::new(merged.clone(), default, None)
    })
    .or_else(|_| {
      tracing::info!("the fetched models no longer take the default model; it is left out");
      NativeModels::new(merged, None, None)
    })
    .map_err(|error| {
      tracing::warn!(%error, "the endpoint listed models that cannot be kept");
      Message::new("connections.models.fetchInvalid")
    })
}

/// Refuses `models` for `connection` while something, as stored now, runs on a model its stored models
/// list and `models` no longer do.
fn ensure_kept(db: &ProjectDb, connection: &AgentConnection, models: &NativeModels) -> Result<(), Message> {
  let id = connection.id();
  let uses = model_uses::load(db).map_err(|error| store_failure(&error))?;
  let before = connection.api().map(|api| api.models().models()).unwrap_or_default();
  keep_used(
    before,
    models.models(),
    &model_uses::of_connection(&uses, id),
    DROPPED_KEYS,
  )
  .inspect_err(|_| {
    tracing::warn!(connection = %id, "something still uses a model the saved models leave out");
  })
}

/// Native connection `id` as stored now.
pub fn load_in(repo_root: &Path, id: ConnectionId) -> Result<AgentConnection, Message> {
  let db = ProjectDb::open(repo_root).map_err(|error| store_failure(&error))?;
  stored(&db, id)
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
    | ConnectionError::EndpointMismatch(_)
    | ConnectionError::NoAccounts(_)
    | ConnectionError::AccountOfOtherCli
    | ConnectionError::NoApiProvider(_)
    | ConnectionError::ApiProviderWithAccount => Message::new("errors.storage"),
  }
}
