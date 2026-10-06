//! A native connection for the runner's tests: the Anthropic API at a fake model server's address, its
//! key a project secret, checked as connected.

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, CheckResult, Connection, ConnectionCheck, EndpointPreset,
  },
  id::{ConnectionId, SecretId},
  name::Name,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  time::Timestamp,
};

/// The model the native Builder runs on.
pub const MODEL: &str = "claude-opus-5-5";
/// The API key of the tests; it must never show up in the activity, approvals or errors.
pub const KEY: &str = "sk-ant-runner-test-4b8e21c9d7";

/// Where a native connection's model API is, and the secret holding its key.
#[derive(Clone, Debug)]
pub struct NativeApi {
  pub base_url: String,
  pub key: SecretId,
}

/// The key secret of a native connection, with its value for the keychain.
pub fn key_secret() -> (Secret, &'static str) {
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("ANTHROPIC_API_KEY").expect("valid name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  );
  (secret, KEY)
}

pub fn connection(api: &NativeApi) -> Connection {
  let endpoint = ApiEndpoint::new(
    EndpointPreset::Official,
    BaseUrl::new(&api.base_url).expect("the fake server's address is a base URL"),
    api.key,
  );
  let mut connection = AgentConnection::native(
    ConnectionId::generate(),
    Name::new("Claude API · Kontur").expect("valid name"),
    AgentProvider::AnthropicApi,
    endpoint,
    Timestamp::now(),
  )
  .expect("a native connection");
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Api { balance: None },
    })
    .expect("an API check fits a native connection");
  connection.into_connection()
}
