//! Setting up a native agent connection (`docs/architecture/native-agent.md` §3.1, §7): the presets
//! this version offers, its models with their default model and effort, provider fallback, and the
//! statuses a test of its model API and its key secret give it.

use std::num::NonZeroU32;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, CheckResult, ConnectionCheck, ConnectionError,
    ConnectionStatus, Effort, EndpointPreset, ModelSource, NativeModel, NativeModels, NativeModelsError,
    offers_provider_fallback,
  },
  id::{ConnectionId, SecretId},
  name::Name,
  resource::ModelId,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  time::Timestamp,
};

fn api(preset: EndpointPreset, url: &str, key: SecretId) -> ApiEndpoint {
  ApiEndpoint::new(preset, BaseUrl::new(url).expect("valid base URL"), key)
}

fn native(provider: AgentProvider, endpoint: ApiEndpoint) -> Result<AgentConnection, ConnectionError> {
  AgentConnection::native(
    ConnectionId::generate(),
    Name::new("Claude API · Kontur").expect("valid name"),
    provider,
    endpoint,
    Timestamp::from_unix_millis(1_000),
  )
}

fn model_id(id: &str) -> ModelId {
  ModelId::new(id).expect("model id")
}

fn model(id: &str, efforts: &[Effort], source: ModelSource) -> NativeModel {
  NativeModel::new(
    model_id(id),
    NonZeroU32::new(200_000),
    NonZeroU32::new(64_000),
    efforts.to_vec(),
    source,
  )
}

fn key_secret() -> Secret {
  Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("ANTHROPIC_API_KEY").expect("secret name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::from_unix_millis(500),
  )
}

#[test]
fn native_models_list_each_id_once_with_a_listed_default_and_one_of_its_efforts() {
  let fetched = ModelSource::Endpoint {
    at: Timestamp::from_unix_millis(5_000),
  };
  let opus = model("claude-opus-5-5", &[Effort::Max, Effort::Low, Effort::Low], fetched);
  assert_eq!(opus.efforts(), [Effort::Low, Effort::Max], "once each, least first");
  let haiku = model("claude-haiku-4-5", &[], ModelSource::Manual);
  let models = NativeModels::new(
    vec![opus.clone(), haiku.clone()],
    Some(model_id("claude-opus-5-5")),
    Some(Effort::Max),
  )
  .expect("valid models");
  assert_eq!(models.default_model(), Some(&model_id("claude-opus-5-5")));
  assert_eq!(models.model(&model_id("claude-haiku-4-5")), Some(&haiku));
  let refused = [
    (
      NativeModels::new(vec![opus.clone(), opus.clone()], None, None),
      NativeModelsError::Duplicate(model_id("claude-opus-5-5")),
    ),
    (
      NativeModels::new(vec![haiku.clone()], Some(model_id("claude-opus-5-5")), None),
      NativeModelsError::DefaultNotListed(model_id("claude-opus-5-5")),
    ),
    (
      NativeModels::new(vec![haiku], Some(model_id("claude-haiku-4-5")), Some(Effort::Low)),
      NativeModelsError::EffortNotOffered(Effort::Low),
    ),
    (
      NativeModels::new(vec![opus], None, Some(Effort::Low)),
      NativeModelsError::EffortWithoutModel,
    ),
  ];
  for (models, error) in refused {
    assert_eq!(models, Err(error));
  }
  // Stored models are checked again when read back.
  let json = serde_json::to_string(&models).expect("serializes");
  assert_eq!(serde_json::from_str::<NativeModels>(&json).expect("reads back"), models);
  let broken = json.replace(r#""default_effort":"max""#, r#""default_effort":"medium""#);
  assert_ne!(broken, json);
  assert!(serde_json::from_str::<NativeModels>(&broken).is_err());
}

#[test]
fn only_the_official_presets_can_be_set_up_and_each_sets_its_base_url() {
  assert!(AgentProvider::AnthropicApi.can_be_added());
  assert!(!AgentProvider::OpenAiApi.can_be_added());
  assert!(AgentProvider::CLI.iter().all(|provider| provider.can_be_added()));
  assert!(EndpointPreset::Official.is_available());
  assert!(!EndpointPreset::DeepSeek.is_available());
  assert!(!EndpointPreset::Custom.is_available());
  assert!(!EndpointPreset::DeepSeek.offered_on(AgentProvider::AnthropicApi));
  assert!(EndpointPreset::DeepSeek.offered_on(AgentProvider::OpenAiApi));
  assert!(EndpointPreset::DeepSeek.has_data_notice());
  assert!(!EndpointPreset::Official.has_data_notice());
  let url = |preset: EndpointPreset, provider| preset.base_url(provider).map(|url| url.as_str().to_owned());
  assert_eq!(
    url(EndpointPreset::Official, AgentProvider::AnthropicApi).as_deref(),
    Some("https://api.anthropic.com")
  );
  assert_eq!(
    url(EndpointPreset::Official, AgentProvider::OpenAiApi).as_deref(),
    Some("https://api.openai.com/v1")
  );
  assert_eq!(
    url(EndpointPreset::DeepSeek, AgentProvider::OpenAiApi).as_deref(),
    Some("https://api.deepseek.com")
  );
  assert_eq!(url(EndpointPreset::DeepSeek, AgentProvider::AnthropicApi), None);
  assert_eq!(url(EndpointPreset::Custom, AgentProvider::AnthropicApi), None);
}

#[test]
fn provider_fallback_is_off_and_offered_on_the_official_anthropic_api_only() {
  let official = api(
    EndpointPreset::Official,
    "https://api.anthropic.com",
    SecretId::generate(),
  );
  assert!(!official.provider_fallback());
  assert!(offers_provider_fallback(
    AgentProvider::AnthropicApi,
    EndpointPreset::Official
  ));
  let on = native(AgentProvider::AnthropicApi, official.with_provider_fallback(true)).expect("offered");
  assert!(on.api().is_some_and(ApiEndpoint::provider_fallback));
  for (provider, preset, url) in [
    (
      AgentProvider::AnthropicApi,
      EndpointPreset::Custom,
      "https://gateway.example.dev",
    ),
    (
      AgentProvider::OpenAiApi,
      EndpointPreset::Official,
      "https://api.openai.com/v1",
    ),
  ] {
    assert!(!offers_provider_fallback(provider, preset));
    let endpoint = api(preset, url, SecretId::generate()).with_provider_fallback(true);
    assert_eq!(native(provider, endpoint), Err(ConnectionError::FallbackNotOffered));
  }
}

#[test]
fn a_native_connection_takes_api_checks_and_reads_key_missing_without_its_secret() {
  let key = key_secret();
  let endpoint = api(EndpointPreset::Official, "https://api.anthropic.com", key.id());
  let mut connection = native(AgentProvider::AnthropicApi, endpoint).expect("native connection");
  let active = std::slice::from_ref(&key);
  assert_eq!(connection.status_with(active), ConnectionStatus::NotChecked);
  assert_eq!(connection.status_with(&[]), ConnectionStatus::KeyMissing);
  let results = [
    (
      CheckResult::Api {
        balance: Some("12.40".into()),
      },
      ConnectionStatus::Connected,
    ),
    (CheckResult::KeyRejected, ConnectionStatus::KeyRejected),
    (
      CheckResult::BillingProblem {
        balance: Some("0.00".into()),
      },
      ConnectionStatus::BillingProblem,
    ),
    (CheckResult::EndpointUnreachable, ConnectionStatus::EndpointUnreachable),
  ];
  for (result, status) in results {
    assert!(result.is_api() && !result.is_verified());
    let check = ConnectionCheck {
      at: Timestamp::from_unix_millis(3_000),
      result,
    };
    connection.record_check(check).expect("an API check fits");
    assert_eq!(connection.status_with(active), status);
    assert_eq!(connection.status_with(&[]), ConnectionStatus::KeyMissing);
  }
  let billing = CheckResult::BillingProblem {
    balance: Some("0.00".into()),
  };
  assert_eq!(billing.balance(), Some("0.00"));
  // An API check does not describe a CLI.
  let mut cli = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude Code").expect("valid name"),
    AgentProvider::ClaudeCodeCli,
    std::env::temp_dir().join("claude"),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("CLI connection");
  let api_check = ConnectionCheck {
    at: Timestamp::from_unix_millis(3_000),
    result: CheckResult::KeyRejected,
  };
  assert!(matches!(
    cli.record_check(api_check),
    Err(ConnectionError::CheckKindMismatch(_))
  ));
}

#[test]
fn changing_models_or_fallback_keeps_the_last_check_and_another_key_drops_it() {
  let key = key_secret();
  let endpoint = api(EndpointPreset::Official, "https://api.anthropic.com", key.id());
  let mut connection = native(AgentProvider::AnthropicApi, endpoint.clone()).expect("native connection");
  let check = ConnectionCheck {
    at: Timestamp::from_unix_millis(3_000),
    result: CheckResult::EndpointUnreachable,
  };
  connection.record_check(check).expect("an API check fits");
  let models = NativeModels::new(
    vec![model("claude-opus-5-5", &[Effort::High], ModelSource::Manual)],
    Some(model_id("claude-opus-5-5")),
    Some(Effort::High),
  )
  .expect("models");
  connection
    .set_api(
      endpoint
        .clone()
        .with_models(models.clone())
        .with_provider_fallback(true),
    )
    .expect("the same API");
  assert_eq!(connection.status(), ConnectionStatus::EndpointUnreachable);
  assert_eq!(connection.api().map(ApiEndpoint::models), Some(&models));
  let other_key = ApiEndpoint::new(
    EndpointPreset::Official,
    endpoint.base_url().clone(),
    SecretId::generate(),
  );
  connection.set_api(other_key).expect("another key");
  assert_eq!(connection.status(), ConnectionStatus::NotChecked);
  let custom = api(EndpointPreset::Custom, "https://gateway.example.dev", key.id()).with_provider_fallback(true);
  assert_eq!(connection.set_api(custom), Err(ConnectionError::FallbackNotOffered));
}
