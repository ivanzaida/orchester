//! Agent CLI connections on API providers (`docs/design/api-providers.md`): the provider's rules, what a connection
//! on one offers and reads, and how each CLI is started on it with the key only in a secret variable.

use std::{path::Path, sync::Arc};

use orchester_core::{
  command::Command,
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, ApiProvider, ApiProviderError, BaseUrl, CatalogSource, CheckResult,
    Connection, ConnectionAccount, ConnectionCheck, ConnectionError, ConnectionKind, ConnectionStatus, EndpointPreset,
    EngineVendor, GitHubHost, LastCheck, ModelCatalog, SignIn, WireApi,
    invocation::{CLAUDE_KEY_VARIABLE, CLAUDE_MODEL_VARIABLES, CODEX_KEY_VARIABLE, OnProvider},
  },
  id::{AgentAccountId, ConnectionId, SecretId},
  name::Name,
  resource::ModelId,
  run::review::reviewer_kind,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue},
  time::Timestamp,
};

/// A fake key; it must never show up anywhere but the secret variable.
const KEY: &str = "sk-fake-provider-key-7f3a91c2";

fn models(ids: &[&str]) -> Vec<ModelId> {
  ids.iter().map(|id| ModelId::new(id).expect("valid model id")).collect()
}

fn provider(url: &str, key: SecretId) -> ApiProvider {
  ApiProvider::new(
    BaseUrl::new(url).expect("valid base URL"),
    key,
    models(&["deepseek-flash", "deepseek-v4-pro"]),
  )
  .expect("a provider with models")
}

fn cli(provider: AgentProvider) -> AgentConnection {
  AgentConnection::new(
    ConnectionId::generate(),
    Name::new("DeepSeek · Claude Code").expect("valid name"),
    provider,
    std::env::temp_dir().join("claude"),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("an absolute program")
}

fn signed_out() -> ConnectionCheck {
  ConnectionCheck {
    at: Timestamp::from_unix_millis(2_000),
    result: CheckResult::Available {
      version: "2.1.301".to_owned(),
      sign_in: SignIn::SignedOut,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  }
}

fn key_secret(id: SecretId) -> Secret {
  Secret::new(
    id,
    NewSecret {
      name: SecretName::new("DEEPSEEK_KEY").expect("valid name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::from_unix_millis(1_000),
  )
}

#[test]
fn api_providers_need_a_model_and_keep_each_once_in_order() {
  let url = BaseUrl::new("https://api.deepseek.com/anthropic").expect("valid");
  assert_eq!(
    ApiProvider::new(url.clone(), SecretId::generate(), Vec::new()),
    Err(ApiProviderError::NoModels)
  );
  let provider = ApiProvider::new(url, SecretId::generate(), models(&["b", "a", "b"])).expect("models");
  let ids: Vec<&str> = provider.model_ids().map(ModelId::as_str).collect();
  assert_eq!(ids, ["b", "a"]);
  assert!(provider.models().iter().all(|model| model.efforts().is_empty()));
}

#[test]
fn api_providers_are_stored_as_ids_and_refused_without_models() {
  let key = SecretId::generate();
  let stored = serde_json::to_value(provider("https://api.deepseek.com", key)).expect("serializes");
  assert_eq!(
    stored,
    serde_json::json!({
      "base_url": "https://api.deepseek.com",
      "key": key.to_string(),
      "models": ["deepseek-flash", "deepseek-v4-pro"],
    })
  );
  assert_eq!(
    serde_json::from_value::<ApiProvider>(stored).expect("reads back"),
    provider("https://api.deepseek.com", key)
  );
  let empty = serde_json::json!({"base_url": "https://api.deepseek.com", "key": key.to_string(), "models": []});
  assert!(serde_json::from_value::<ApiProvider>(empty).is_err());
}

#[test]
fn api_providers_name_the_wire_each_cli_speaks() {
  assert_eq!(
    AgentProvider::ClaudeCodeCli.provider_wire(),
    Some(WireApi::AnthropicMessages)
  );
  assert_eq!(AgentProvider::CodexCli.provider_wire(), Some(WireApi::OpenAiResponses));
  assert_eq!(AgentProvider::AnthropicApi.provider_wire(), None);
  assert_eq!(AgentProvider::OpenAiApi.provider_wire(), None);
}

#[test]
fn api_providers_run_only_agent_clis_on_their_default_account() {
  let github = Connection::new(
    ConnectionId::generate(),
    Name::new("GitHub").expect("valid"),
    ConnectionKind::GitHub {
      host: GitHubHost::new("github.com").expect("valid host"),
    },
    orchester_core::connection::AgentEndpoint::Program(std::env::temp_dir().join("gh")),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("a GitHub connection");
  let mut github = github;
  assert!(matches!(
    github.set_api_provider(Some(provider("https://api.deepseek.com", SecretId::generate()))),
    Err(ConnectionError::NoApiProvider(_))
  ));
  let mut native = AgentConnection::native(
    ConnectionId::generate(),
    Name::new("Anthropic").expect("valid"),
    AgentProvider::AnthropicApi,
    ApiEndpoint::new(
      EndpointPreset::Official,
      BaseUrl::new("https://api.anthropic.com").expect("valid"),
      SecretId::generate(),
    ),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("a native connection");
  assert!(matches!(
    native.set_api_provider(Some(provider("https://api.deepseek.com", SecretId::generate()))),
    Err(ConnectionError::NoApiProvider(_))
  ));

  let account = ConnectionAccount::Account {
    id: AgentAccountId::generate(),
    folder: std::env::temp_dir().join("claude-work"),
  };
  let mut on_account = cli(AgentProvider::ClaudeCodeCli);
  on_account.set_account(account.clone()).expect("an account of the CLI");
  assert_eq!(
    on_account.set_api_provider(Some(provider(
      "https://api.deepseek.com/anthropic",
      SecretId::generate()
    ))),
    Err(ConnectionError::ApiProviderWithAccount)
  );
  let mut on_provider = cli(AgentProvider::ClaudeCodeCli);
  on_provider
    .set_api_provider(Some(provider(
      "https://api.deepseek.com/anthropic",
      SecretId::generate(),
    )))
    .expect("on Default");
  assert_eq!(
    on_provider.set_account(account),
    Err(ConnectionError::ApiProviderWithAccount)
  );
  on_provider.set_api_provider(None).expect("back to the sign-in");
  assert!(on_provider.api_provider().is_none());
}

#[test]
fn api_providers_give_the_models_status_vendor_and_key_of_their_connection() {
  let key = SecretId::generate();
  let mut connection = cli(AgentProvider::ClaudeCodeCli);
  connection.record_check(signed_out()).expect("a CLI check");
  assert_eq!(connection.status(), ConnectionStatus::SignInRequired);
  assert!(
    connection.plan_account().is_some(),
    "Default's limits are read on the sign-in"
  );

  connection
    .set_api_provider(Some(provider("https://api.deepseek.com/anthropic", key)))
    .expect("on Default");
  assert_eq!(
    connection.status(),
    ConnectionStatus::Connected,
    "the CLI's own sign-in is not used"
  );
  let offered: Vec<&str> = connection.models().iter().map(|model| model.id().as_str()).collect();
  assert_eq!(offered, ["deepseek-flash", "deepseek-v4-pro"]);
  assert!(connection.catalog().is_none(), "the CLI's own catalog does not apply");
  assert_eq!(connection.vendor(), EngineVendor::Host("api.deepseek.com".to_owned()));
  assert_eq!(connection.key(), Some(key));
  assert_eq!(connection.plan_account(), None, "a key has no plan limits");
  assert_eq!(connection.status_with(&[]), ConnectionStatus::KeyMissing);
  assert_eq!(connection.status_with(&[key_secret(key)]), ConnectionStatus::Connected);
  assert_eq!(reviewer_kind(&connection), None, "no reviewer runs on an API provider");
  let stored: Connection = connection.clone().into_connection();
  assert!(matches!(stored.last_check(), Some(LastCheck::Agent(_))));
}

#[test]
fn api_providers_start_claude_code_with_their_url_models_and_the_key_in_a_secret_only() {
  let provider = provider("https://api.deepseek.com/anthropic", SecretId::generate());
  let model = ModelId::new("deepseek-flash").expect("valid");
  let on = OnProvider {
    provider: &provider,
    model: &model,
    key: Arc::new(SecretValue::new(KEY.to_owned()).expect("not blank")),
  };
  let project = [
    ("PATH", "bin"),
    ("ANTHROPIC_API_KEY", "the project's own"),
    ("anthropic_base_url", "https://elsewhere.example"),
    ("ANTHROPIC_CUSTOM_HEADERS", "X-Key: elsewhere"),
    ("ANTHROPIC_BEDROCK_BASE_URL", "https://bedrock.example"),
    ("CLAUDE_CODE_USE_BEDROCK", "1"),
    ("claude_code_use_vertex", "1"),
    ("CLAUDE_CODE_SKIP_FOUNDRY_AUTH", "1"),
    ("CLAUDE_CODE_USE_POWERSHELL_TOOL", "1"),
  ];
  let before = Command::new(Path::new("claude"), ["-p"]).with_environment(
    project
      .iter()
      .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
      .collect(),
  );
  let command = on.apply(before, AgentProvider::ClaudeCodeCli);
  let mut env: Vec<(String, String)> = command.env().to_vec();
  env.sort();
  let mut expected: Vec<(String, String)> = CLAUDE_MODEL_VARIABLES
    .iter()
    .map(|name| ((*name).to_owned(), "deepseek-flash".to_owned()))
    .chain(
      [
        ("ANTHROPIC_BASE_URL", "https://api.deepseek.com/anthropic"),
        ("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "1"),
        ("CLAUDE_CODE_USE_POWERSHELL_TOOL", "1"),
        ("PATH", "bin"),
      ]
      .map(|(name, value)| (name.to_owned(), value.to_owned())),
    )
    .collect();
  expected.sort();
  assert_eq!(
    env, expected,
    "no other credential, header, base URL or cloud routing reaches the provider's host"
  );
  assert_eq!(command.args(), ["-p"]);
  let secrets: Vec<&str> = command.secret_env().iter().map(|secret| secret.name()).collect();
  assert_eq!(secrets, [CLAUDE_KEY_VARIABLE]);
  assert_eq!(command.secret_env()[0].expose(), KEY);
  assert!(
    !format!("{command:?}").contains(KEY),
    "the command's Debug shows no key"
  );
  assert!(
    !format!("{on:?}").contains(KEY),
    "the provider start's Debug shows no key"
  );
}

#[test]
fn api_providers_start_codex_on_a_model_provider_of_their_own() {
  let provider = provider("https://api.deepseek.com", SecretId::generate());
  let model = ModelId::new("deepseek-v4-pro").expect("valid");
  let on = OnProvider {
    provider: &provider,
    model: &model,
    key: Arc::new(SecretValue::new(KEY.to_owned()).expect("not blank")),
  };
  let command = on.apply(
    Command::new(Path::new("codex"), ["app-server", "--stdio"]),
    AgentProvider::CodexCli,
  );
  assert_eq!(
    command.args(),
    [
      "app-server",
      "--stdio",
      "-c",
      "model_provider=\"orchester\"",
      "-c",
      "model_providers.orchester={name=\"Orchester API provider\",base_url=\"https://api.deepseek.com\",\
       env_key=\"ORCHESTER_PROVIDER_API_KEY\",wire_api=\"responses\"}",
      "-c",
      "shell_environment_policy.ignore_default_excludes=false",
    ]
  );
  let secrets: Vec<&str> = command.secret_env().iter().map(|secret| secret.name()).collect();
  assert_eq!(secrets, [CODEX_KEY_VARIABLE]);
  assert!(command.env().is_empty(), "the key is in no plain variable");
  assert!(!command.args().iter().any(|arg| arg.contains(KEY)));
  assert!(!format!("{command:?}").contains(KEY));
}

#[test]
fn api_providers_a_users_codex_provider_of_the_same_id_refuses_the_step() {
  use orchester_core::connection::app_server_config::{ConfigReadError, refuse_taken_provider};
  let layers = |own: serde_json::Value| {
    serde_json::json!({"layers": [
      {"name": {"type": "sessionFlags"}, "config": {"model_providers": {"orchester": {"base_url": "https://api.deepseek.com"}}}},
      {"name": {"type": "user"}, "config": own},
    ]})
  };
  assert_eq!(
    refuse_taken_provider(
      &layers(serde_json::json!({"model_providers": {"mine": {}}})),
      "orchester"
    ),
    Ok(()),
    "Orchester's own overrides and other providers are fine"
  );
  assert_eq!(
    refuse_taken_provider(
      &layers(serde_json::json!({"model_providers": {"orchester": {}}})),
      "orchester"
    ),
    Err(ConfigReadError::ProviderNameTaken("orchester".to_owned()))
  );
}

#[test]
fn api_providers_a_step_stopped_at_a_plan_limit_does_not_continue_on_a_key() {
  let signed_in = ConnectionCheck {
    at: Timestamp::from_unix_millis(2_000),
    result: CheckResult::Available {
      version: "0.159.2".to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  };
  let stopped = Some(AgentAccountId::generate());
  let mut other_account = cli(AgentProvider::CodexCli);
  other_account
    .set_account(ConnectionAccount::Account {
      id: AgentAccountId::generate(),
      folder: std::env::temp_dir(),
    })
    .expect("an account of the CLI");
  other_account.record_check(signed_in.clone()).expect("a CLI check");
  assert!(other_account.continues_plan_limited(AgentProvider::CodexCli, stopped));
  let mut on_key = cli(AgentProvider::CodexCli);
  on_key.record_check(signed_in).expect("a CLI check");
  assert!(
    on_key.continues_plan_limited(AgentProvider::CodexCli, stopped),
    "Default is another account"
  );
  on_key
    .set_api_provider(Some(provider("https://api.deepseek.com", SecretId::generate())))
    .expect("on Default");
  assert!(
    !on_key.continues_plan_limited(AgentProvider::CodexCli, stopped),
    "a key is not another account"
  );
}

#[test]
fn api_providers_a_session_continues_only_on_the_provider_and_account_it_was_made_on() {
  use orchester_core::connection::SessionOrigin;
  let (key, other_key) = (SecretId::generate(), SecretId::generate());
  let url = |value: &str| BaseUrl::new(value).expect("valid");
  let on = |provider: Option<(&str, SecretId)>, account: Option<AgentAccountId>| SessionOrigin {
    account,
    provider: provider.map(|(value, key)| (url(value), key)),
  };
  let work = Some(AgentAccountId::generate());
  let deepseek = Some(("https://api.deepseek.com/anthropic", key));
  assert!(on(deepseek, None).continues_on(&on(deepseek, None)));
  assert!(
    !on(deepseek, None).continues_on(&on(Some(("https://gateway.example.com", key)), None)),
    "another host"
  );
  assert!(
    !on(deepseek, None).continues_on(&on(Some(("https://api.deepseek.com/anthropic", other_key)), None)),
    "another key"
  );
  assert!(!on(deepseek, None).continues_on(&on(None, work)), "back on the sign-in");
  assert!(
    !on(None, work).continues_on(&on(deepseek, None)),
    "from the sign-in to a key"
  );
  assert!(on(None, work).continues_on(&on(None, work)));
  assert!(
    !on(None, work).continues_on(&on(None, Some(AgentAccountId::generate()))),
    "another account"
  );
  assert!(
    on(None, None).continues_on(&on(None, work)),
    "no account recorded: the folder is the same"
  );

  let mut connection = cli(AgentProvider::ClaudeCodeCli);
  connection
    .set_api_provider(Some(provider("https://api.deepseek.com/anthropic", key)))
    .expect("on Default");
  assert_eq!(
    SessionOrigin::of(&connection),
    on(Some(("https://api.deepseek.com/anthropic", key)), None)
  );
}
