//! *Test connection* and *Fetch models* through the runner (`docs/architecture/native-agent.md` §7): the
//! runner reads the key itself, and answers with what the model API's listing found and its models.

use orchester_core::{
  connection::{AgentProvider, ApiEndpoint, BaseUrl, CheckResult, Effort, EndpointPreset, ModelSource},
  id::SecretId,
};
use orchester_ipc::Rejection;

use crate::{
  events::MODEL,
  native_project, serve,
  server::Script,
  support::{native::KEY, scheduler_with},
};

fn api(base_url: &str, key: SecretId) -> ApiEndpoint {
  ApiEndpoint::new(EndpointPreset::Official, BaseUrl::new(base_url).expect("base URL"), key)
}

#[tokio::test(flavor = "multi_thread")]
async fn native_check_lists_the_models() {
  let server = serve(Script::new()).await;
  let project = native_project("Check.", None, &server);
  let root = project.repo.path();
  let (scheduler, _events) = scheduler_with(root, project.vault.clone());
  let key = connection_key(root);
  let (result, models) = scheduler
    .check_connection(AgentProvider::AnthropicApi, api(&server.base_url(), key))
    .await
    .expect("checked");
  assert_eq!(result, CheckResult::Api { balance: None });
  assert_eq!(models.len(), 1);
  assert_eq!(models[0].id().as_str(), MODEL);
  assert_eq!(models[0].efforts(), Effort::ALL);
  assert!(matches!(models[0].source(), ModelSource::Endpoint { .. }));
  let request = &server.requests()[0];
  assert_eq!(request.header("x-api-key"), Some(KEY));
  assert!(request.path.starts_with("/v1/models"));
}

#[tokio::test(flavor = "multi_thread")]
async fn native_check_tells_a_rejected_key_and_a_missing_one() {
  let body = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
  let server = serve(Script::new().reply_status("/v1/models", 401, &[], body)).await;
  let project = native_project("Check.", None, &server);
  let root = project.repo.path();
  let (scheduler, _events) = scheduler_with(root, project.vault.clone());
  let key = connection_key(root);
  let (result, models) = scheduler
    .check_connection(AgentProvider::AnthropicApi, api(&server.base_url(), key))
    .await
    .expect("checked");
  assert_eq!(result, CheckResult::KeyRejected);
  assert!(models.is_empty());
  let missing = scheduler
    .check_connection(
      AgentProvider::AnthropicApi,
      api(&server.base_url(), SecretId::generate()),
    )
    .await;
  assert!(matches!(missing, Err(Rejection::Failed { detail }) if !detail.contains(KEY)));
  let openai = scheduler
    .check_connection(AgentProvider::OpenAiApi, api("https://api.openai.com/v1", key))
    .await;
  assert!(matches!(openai, Err(Rejection::Failed { .. })));
}

/// The key secret of the project's native connection.
fn connection_key(root: &std::path::Path) -> SecretId {
  use orchester_core::store::SecretStore;
  let db = orchester_store::ProjectDb::open(root).expect("open");
  db.secrets().expect("secrets")[0].id()
}
