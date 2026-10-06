//! Model listing and *Test connection*: `GET /models` with the built-in table's limits and efforts, and
//! the check's results.

use orchester_agent::check_responses;
use orchester_core::{
  connection::CheckResult,
  native::model::{ModelClient, ModelError},
};

use crate::{
  KEY, client, endpoint,
  events::{self, MODEL},
  server::{FakeServer, Script, Wire},
  start,
};

#[tokio::test]
async fn responses_models_are_listed_with_the_table_metadata() {
  let catalog = vec![
    events::listed("gpt-6-luna"),
    events::listed(MODEL),
    events::listed("ft:gpt-6-luna:lab::abc123"),
    events::listed("not a model id"),
  ];
  let server = FakeServer::start_wire(Wire::Responses, Script::new(), catalog, 100).await;
  let models = client(&server).models().await.expect("models");
  let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
  assert_eq!(ids, ["gpt-6-luna", MODEL, "ft:gpt-6-luna:lab::abc123"]);
  assert_eq!(models[0].efforts, ["none", "low", "medium", "high", "xhigh", "max"]);
  assert_eq!(models[1].efforts, ["low", "medium", "high", "xhigh", "max"]);
  assert_eq!(models[1].context_window, Some(1_050_000));
  assert_eq!(models[1].max_output, Some(128_000));
  assert_eq!(models[2].context_window, None);
  assert_eq!(models[2].max_output, None);
  assert!(models[2].efforts.is_empty());
  let requests = server.requests();
  assert_eq!(requests.len(), 1);
  assert_eq!(requests[0].path, "/v1/models");
  assert_eq!(
    requests[0].header("authorization"),
    Some(format!("Bearer {KEY}").as_str())
  );
  server.finish();
}

#[tokio::test]
async fn responses_check_reports_the_endpoint_state() {
  let server = start(Script::new()).await;
  let check = check_responses(endpoint(&server)).await;
  assert_eq!(check.result, CheckResult::Api { balance: None });
  assert_eq!(check.models.len(), 2);
  server.finish();

  let body = r#"{"error":{"message":"Incorrect API key provided.","type":"invalid_request_error","param":null,"code":"invalid_api_key"}}"#;
  let rejected = start(Script::new().reply_status("/v1/models", 401, &[], body)).await;
  let check = check_responses(endpoint(&rejected)).await;
  assert_eq!(check.result, CheckResult::KeyRejected);
  assert_eq!(check.error, Some(ModelError::KeyRejected));
  rejected.finish();

  let body = r#"{"error":{"message":"You exceeded your current quota.","type":"insufficient_quota","param":null,"code":"insufficient_quota"}}"#;
  let quota = start(Script::new().reply_status("/v1/models", 429, &[], body)).await;
  let check = check_responses(endpoint(&quota)).await;
  assert_eq!(check.result, CheckResult::BillingProblem { balance: None });
  quota.finish();

  let closed = start(Script::new()).await;
  let gone = endpoint(&closed);
  drop(closed);
  let check = check_responses(gone).await;
  assert_eq!(check.result, CheckResult::EndpointUnreachable);
}
