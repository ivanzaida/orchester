//! Model listing and *Test connection*: the Models API read page by page, and the check's results.

use orchester_agent::{WireQuirks, check_anthropic};
use orchester_core::{
  connection::CheckResult,
  native::model::{ModelClient, ModelError},
};

use crate::{
  KEY, client, endpoint,
  events::{self, MODEL},
  server::{FakeServer, Script},
};

#[tokio::test]
async fn anthropic_models_are_listed_across_pages() {
  let catalog = vec![
    events::model(MODEL, true, 128_000, &["max", "low", "high"]),
    events::model("claude-sonnet-5-5", true, 128_000, &["medium"]),
    events::model("claude-haiku-4-5", false, 64_000, &[]),
  ];
  let server = FakeServer::start(Script::new(), catalog, 2).await;
  let models = client(&server).models().await.expect("models");
  let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
  assert_eq!(ids, [MODEL, "claude-sonnet-5-5", "claude-haiku-4-5"]);
  assert_eq!(models[0].efforts, ["low", "high", "max"]);
  assert_eq!(models[0].context_window, Some(1_000_000));
  assert_eq!(models[0].max_output, Some(128_000));
  assert!(models[2].efforts.is_empty());
  let listed = server
    .requests()
    .iter()
    .filter(|request| request.path.starts_with("/v1/models?"))
    .count();
  assert_eq!(listed, 2);
  assert!(
    server
      .requests()
      .iter()
      .all(|request| request.header("x-api-key") == Some(KEY))
  );
  server.finish();
}

#[tokio::test]
async fn anthropic_check_reports_the_endpoint_state() {
  let server = FakeServer::start(Script::new(), events::catalog(), 10).await;
  let check = check_anthropic(endpoint(&server, WireQuirks::OFFICIAL)).await;
  assert_eq!(check.result, CheckResult::Api { balance: None });
  assert_eq!(check.models.len(), 1);
  server.finish();

  let body = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
  let rejected = FakeServer::start(Script::new().reply_status("/v1/models", 401, &[], body), Vec::new(), 10).await;
  let check = check_anthropic(endpoint(&rejected, WireQuirks::OFFICIAL)).await;
  assert_eq!(check.result, CheckResult::KeyRejected);
  assert_eq!(check.error, Some(ModelError::KeyRejected));
  rejected.finish();

  let closed = FakeServer::start(Script::new(), Vec::new(), 10).await;
  let gone = endpoint(&closed, WireQuirks::OFFICIAL);
  drop(closed);
  let check = check_anthropic(gone).await;
  assert_eq!(check.result, CheckResult::EndpointUnreachable);
}
