//! API accounts (`docs/design/agent-accounts.md`, "Agents run on accounts"): what a setup takes, when a check is
//! kept, and where the key is in the keychain.

use orchester_core::{
  account::{AccountCli, ApiAccount, ApiAccountError, ApiSetup},
  connection::{
    AgentProvider, BaseUrl, CatalogSource, CheckResult, ConnectionCheck, EndpointPreset, ModelCatalog, NativeModels,
    SignIn,
  },
  id::{ApiAccountId, ProjectId, SecretId},
  name::Name,
  resource::ModelId,
  secret::VaultKey,
  time::Timestamp,
};

fn url(text: &str) -> BaseUrl {
  BaseUrl::new(text).expect("a base URL")
}

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("a model id")
}

fn native(preset: EndpointPreset, base_url: &str, fallback: bool) -> Result<ApiSetup, ApiAccountError> {
  ApiSetup::native(
    AgentProvider::AnthropicApi,
    preset,
    url(base_url),
    NativeModels::default(),
    fallback,
  )
}

fn account(setup: ApiSetup) -> ApiAccount {
  ApiAccount::new(
    ApiAccountId::generate(),
    Name::new("Team key").expect("a name"),
    setup,
    Timestamp::from_unix_millis(1),
  )
}

fn api_check(at: i64) -> ConnectionCheck {
  ConnectionCheck {
    at: Timestamp::from_unix_millis(at),
    result: CheckResult::Api { balance: None },
  }
}

#[test]
fn api_account_a_cli_setup_keeps_each_model_once_and_needs_one() {
  let setup = ApiSetup::cli(
    AccountCli::ClaudeCode,
    url("https://api.deepseek.com/anthropic"),
    vec![
      model("deepseek-chat"),
      model("deepseek-reasoner"),
      model("deepseek-chat"),
    ],
  )
  .expect("a setup");
  assert_eq!(setup.agent(), AgentProvider::ClaudeCodeCli);
  assert!(
    matches!(&setup, ApiSetup::Cli { models, .. } if *models == [model("deepseek-chat"), model("deepseek-reasoner")])
  );

  let none = ApiSetup::cli(AccountCli::Codex, url("https://api.deepseek.com"), Vec::new());
  assert_eq!(none, Err(ApiAccountError::NoModels));
}

#[test]
fn api_account_a_native_setup_is_for_a_model_api_with_what_it_offers() {
  let cli = ApiSetup::native(
    AgentProvider::ClaudeCodeCli,
    EndpointPreset::Custom,
    url("https://gateway.example"),
    NativeModels::default(),
    false,
  );
  assert_eq!(cli, Err(ApiAccountError::NotNative(AgentProvider::ClaudeCodeCli)));
  assert_eq!(
    native(EndpointPreset::DeepSeek, "https://api.deepseek.com", false),
    Err(ApiAccountError::PresetNotOffered)
  );
  assert_eq!(
    native(EndpointPreset::Custom, "https://gateway.example", true),
    Err(ApiAccountError::FallbackNotOffered)
  );
  assert!(native(EndpointPreset::Official, "https://api.anthropic.com", true).is_ok());
}

#[test]
fn api_account_only_a_model_api_takes_a_check_and_only_of_an_api() {
  let mut on_cli = account(
    ApiSetup::cli(
      AccountCli::Codex,
      url("https://api.deepseek.com"),
      vec![model("deepseek-chat")],
    )
    .expect("a setup"),
  );
  assert_eq!(on_cli.record_check(api_check(2)), Err(ApiAccountError::CheckNotTaken));

  let mut own = account(native(EndpointPreset::Official, "https://api.anthropic.com", false).expect("a setup"));
  let of_a_program = ConnectionCheck {
    at: Timestamp::from_unix_millis(2),
    result: CheckResult::Available {
      version: "2.1.312".to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  };
  assert_eq!(own.record_check(of_a_program), Err(ApiAccountError::CheckNotTaken));
  own.record_check(api_check(3)).expect("an API check");
  assert_eq!(own.last_check(), Some(&api_check(3)));
}

#[test]
fn api_account_the_check_stays_while_the_api_does() {
  let mut own = account(native(EndpointPreset::Official, "https://api.anthropic.com", false).expect("a setup"));
  own.record_check(api_check(2)).expect("an API check");

  own
    .set_setup(native(EndpointPreset::Official, "https://api.anthropic.com", true).expect("a setup"))
    .expect("the same API with provider fallback on");
  assert_eq!(own.last_check(), Some(&api_check(2)));

  own
    .set_setup(native(EndpointPreset::Custom, "https://gateway.example", false).expect("a setup"))
    .expect("another API");
  assert_eq!(own.last_check(), None);

  own.record_check(api_check(4)).expect("an API check");
  own.key_replaced();
  assert_eq!(own.last_check(), None, "the check was made with the key before");

  let other_agent =
    ApiSetup::cli(AccountCli::ClaudeCode, url("https://gateway.example"), vec![model("m")]).expect("a setup");
  assert_eq!(own.set_setup(other_agent), Err(ApiAccountError::AnotherAgent));
}

#[test]
fn api_account_key_is_kept_apart_from_every_project_secret() {
  let id = ApiAccountId::generate();
  let key = VaultKey::api_account(id);
  assert_eq!(key.service(), format!("orchester-api-account:{id}"));
  assert_eq!(key.account(), "key");

  let as_secret = VaultKey::new(ProjectId::from_uuid(id.as_uuid()), SecretId::from_uuid(id.as_uuid()));
  assert_ne!(key, as_secret);
}
