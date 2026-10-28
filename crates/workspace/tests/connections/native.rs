//! Native connections as Add provider and Edit connection save them (`docs/architecture/native-agent.md` §7): a new
//! key secret created with the connection, its models kept while a profile runs on one, a test's fetched models merged
//! in, and the models and efforts a profile chooses from.

use std::num::NonZeroU32;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CheckResult, ConnectionStatus, Effort, EndpointPreset, ModelSource, NativeModel,
    NativeModels,
  },
  id::ConnectionId,
  name::Name,
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings},
  secret::{Environment, NewSecret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::SecretStore,
  time::Timestamp,
};
use orchester_workspace::{
  agents::{
    data::create_profile,
    form::{effort_options, first_model, listed_models},
  },
  connections::native::{
    KeySecret, NativeDraft, load_in, record_check_in, record_tested_in, save_models_in, save_native_in,
  },
};

use super::{now, project_db};

/// Takes every key it is given and gives none back: these tests never read one.
struct Keychain;

impl SecretVault for Keychain {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Ok(())
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("a valid model id")
}

fn manual(id: &str) -> NativeModel {
  NativeModel::new(model(id), None, None, Vec::new(), ModelSource::Manual)
}

/// An Anthropic API connection on the official preset with a new key secret and `models`.
fn draft(id: Option<ConnectionId>, key: KeySecret, models: NativeModels) -> NativeDraft {
  NativeDraft {
    id,
    name: Name::new("Anthropic · Kontur").expect("a valid name"),
    provider: AgentProvider::AnthropicApi,
    preset: EndpointPreset::Official,
    base_url: EndpointPreset::Official
      .base_url(AgentProvider::AnthropicApi)
      .expect("the official URL"),
    key,
    models,
    provider_fallback: false,
  }
}

fn new_key() -> KeySecret {
  KeySecret::New(NewSecret {
    name: SecretName::new("ANTHROPIC_API_KEY").expect("a valid name"),
    kind: SecretKind::ApiKey,
    environment: Environment::NEW_SECRET,
  })
}

#[test]
fn native_connection_saves_its_new_key_secret_and_keeps_a_model_a_profile_runs_on() {
  let (repo, mut db) = project_db();
  let models = NativeModels::new(
    vec![manual("claude-opus-5-5"), manual("claude-haiku-5-5")],
    Some(model("claude-opus-5-5")),
    None,
  )
  .expect("valid models");
  let value = SecretValue::new("sk-ant-test".to_owned()).expect("a key");
  let id = save_native_in(repo.path(), &Keychain, &draft(None, new_key(), models), Some(value)).expect("saved");
  let connection = load_in(repo.path(), id).expect("stored");
  let api = connection.api().expect("a model API");
  let secrets = db.secrets().expect("secrets read");
  let key = secrets
    .iter()
    .find(|secret| secret.id() == api.key())
    .expect("the key secret");
  assert_eq!(key.name().as_str(), "ANTHROPIC_API_KEY");
  assert_eq!(connection.status_with(&secrets), ConnectionStatus::NotChecked);
  assert_eq!(
    first_model(&connection),
    Some(model("claude-opus-5-5")),
    "the default first"
  );
  assert_eq!(
    listed_models(&connection),
    [model("claude-opus-5-5"), model("claude-haiku-5-5")]
  );

  let profile = AgentProfile::new(
    Name::new("Builder").expect("a valid role"),
    String::new(),
    PromptText::new("").expect("a valid prompt"),
    id,
    model("claude-haiku-5-5"),
    ProviderSettings::default(),
  );
  create_profile(&mut db, &profile, now()).expect("the profile");
  let without = NativeModels::new(vec![manual("claude-opus-5-5")], None, None).expect("valid models");
  let refused = save_models_in(repo.path(), id, without.clone()).expect_err("a used model stays");
  assert_eq!(refused.key, "connections.models.usedDropped");
  let refused = save_native_in(
    repo.path(),
    &Keychain,
    &draft(Some(id), KeySecret::Existing(api.key()), without),
    None,
  )
  .expect_err("Edit connection keeps it too");
  assert_eq!(refused.key, "connections.models.usedDropped");
  assert_eq!(
    listed_models(&load_in(repo.path(), id).expect("stored")).len(),
    2,
    "nothing was dropped"
  );
}

#[test]
fn native_connection_test_merges_the_listed_models_with_their_efforts() {
  let (repo, db) = project_db();
  let models =
    NativeModels::new(vec![manual("claude-opus-5-5")], Some(model("claude-opus-5-5")), None).expect("valid models");
  let value = SecretValue::new("sk-ant-test".to_owned()).expect("a key");
  let id = save_native_in(repo.path(), &Keychain, &draft(None, new_key(), models), Some(value)).expect("saved");
  let at = Timestamp::from_unix_millis(1_700_000_100_000);
  let listed = NativeModel::new(
    model("claude-opus-5-5"),
    NonZeroU32::new(1_000_000),
    NonZeroU32::new(128_000),
    vec![Effort::Low, Effort::High, Effort::Max],
    ModelSource::Endpoint { at },
  );
  let fetched = NativeModel::new(
    model("claude-sonnet-5-5"),
    None,
    None,
    vec![Effort::Medium],
    ModelSource::Endpoint { at },
  );
  record_check_in(
    repo.path(),
    id,
    CheckResult::Api { balance: None },
    vec![listed, fetched],
  )
  .expect("recorded");
  let connection: AgentConnection = load_in(repo.path(), id).expect("stored");
  let secrets = db.secrets().expect("secrets read");
  assert_eq!(connection.status_with(&secrets), ConnectionStatus::Connected);
  let api = connection.api().expect("a model API");
  assert_eq!(
    api.models().default_model(),
    Some(&model("claude-opus-5-5")),
    "the default stays"
  );
  assert_eq!(
    listed_models(&connection),
    [model("claude-opus-5-5"), model("claude-sonnet-5-5")]
  );
  assert_eq!(
    effort_options(&connection, &model("claude-opus-5-5")),
    ["low", "high", "max"],
    "a profile chooses among the efforts the endpoint listed"
  );
  assert!(effort_options(&connection, &model("claude-haiku-5-5")).is_empty());
  assert_eq!(
    api.models().model(&model("claude-opus-5-5")).map(NativeModel::source),
    Some(ModelSource::Endpoint { at })
  );
}

#[test]
fn native_connection_test_of_a_key_saved_away_meanwhile_records_nothing() {
  let (repo, db) = project_db();
  let value = SecretValue::new("sk-ant-test".to_owned()).expect("a key");
  let draft_now = draft(None, new_key(), NativeModels::default());
  let id = save_native_in(repo.path(), &Keychain, &draft_now, Some(value)).expect("saved");
  let tested = load_in(repo.path(), id)
    .expect("stored")
    .api()
    .expect("a model API")
    .clone();
  // While the runner tests the old key, the person saves the connection on another one.
  let other = KeySecret::New(NewSecret {
    name: SecretName::new("ANTHROPIC_API_KEY_2").expect("a valid name"),
    kind: SecretKind::ApiKey,
    environment: Environment::NEW_SECRET,
  });
  let value = SecretValue::new("sk-ant-other".to_owned()).expect("a key");
  save_native_in(
    repo.path(),
    &Keychain,
    &draft(Some(id), other, NativeModels::default()),
    Some(value),
  )
  .expect("saved on the other key");
  let stale = record_tested_in(
    repo.path(),
    id,
    &tested,
    CheckResult::KeyRejected,
    vec![manual("claude-opus-5-5")],
  )
  .expect_err("the result is the old key's");
  assert_eq!(stale.key, "connections.native.testStale");
  let connection = load_in(repo.path(), id).expect("stored");
  let secrets = db.secrets().expect("secrets read");
  assert_eq!(
    connection.status_with(&secrets),
    ConnectionStatus::NotChecked,
    "the new key is not said to be rejected"
  );
  assert!(listed_models(&connection).is_empty(), "nothing was merged");

  let now = connection.api().expect("a model API").clone();
  record_tested_in(repo.path(), id, &now, CheckResult::Api { balance: None }, Vec::new())
    .expect("a test of the stored key is recorded");
  let secrets = db.secrets().expect("secrets read");
  assert_eq!(
    load_in(repo.path(), id).expect("stored").status_with(&secrets),
    ConnectionStatus::Connected
  );
}
