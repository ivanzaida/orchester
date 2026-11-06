#[path = "decision_review/server.rs"]
mod server;
mod support;

use std::time::{Duration, Instant};

use orchester_core::{
  connection::{DecisionCheck, DecisionCheckResult, DecisionConnection, DecisionServerUrl},
  id::{DecisionConnectionId, IdempotencyKey, SecretId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::ModelId,
  run::review::{
    AutoModeSettings, Probability, RequestReviewer, Review, ReviewCost, ReviewError, ReviewFailure, ReviewInput,
    ReviewSpend, ReviewVerdict, Reviewed, ReviewedRequest, ReviewedStep, ReviewedTask, ReviewerChoice,
  },
  secret::SecretValue,
  store::ActivityStore,
  time::Timestamp,
};
use orchester_runner::decision_review::DecisionReviewer;
use orchester_store::ProjectDb;
use server::{Server, answer};

fn model() -> ModelId {
  ModelId::new("jev-latest").expect("model")
}

fn connection(url: &str, key: bool) -> DecisionConnection {
  let mut connection = DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new("Local decision").expect("name"),
    DecisionServerUrl::new(url).expect("URL"),
    key.then(SecretId::generate),
  )
  .with_models(vec![model()]);
  connection.record_check(DecisionCheck {
    at: Timestamp::now(),
    result: DecisionCheckResult::Connected,
  });
  connection
}

fn input() -> ReviewInput {
  ReviewInput::new(
    ReviewedRequest {
      command: true,
      server: "shell".to_owned(),
      action: "cargo build".to_owned(),
      arguments: None,
      note: None,
      working_folder: None,
    },
    ReviewedStep {
      name: "Implement".to_owned(),
      repository: RepositoryAccess::Write,
      network: NetworkAccess::On,
      writable_folders: vec![],
    },
    ReviewedTask {
      title: "Build".to_owned(),
      brief: "Build the project".to_owned(),
      criteria: vec![],
    },
    &[],
  )
  .expect("input")
}

async fn review(connection: DecisionConnection, key: Option<&str>) -> Reviewed {
  let reviewer = DecisionReviewer::new(
    tokio::runtime::Handle::current(),
    connection,
    model(),
    Probability::new(0.8).expect("threshold"),
    key.map(|key| SecretValue::new(key.to_owned()).expect("key")),
  );
  tokio::task::spawn_blocking(move || reviewer.review(&input()))
    .await
    .expect("review")
}

#[tokio::test]
async fn decision_review_threshold_and_input_only_usage() {
  for (probability, approved) in [(0.8, true), (0.799, false)] {
    let server = Server::new("200 OK", answer(probability).to_string(), Duration::ZERO, true).await;
    let reviewed = review(connection(&server.url, false), None).await;
    let cost = ReviewCost::of(&reviewed, &model()).expect("cost");
    assert_eq!(cost.model.as_str(), "jev-documentation-example");
    let usage = cost.usage.expect("usage");
    assert_eq!((usage.input, usage.cached_input, usage.output), (120, 0, 0));
    let review = Review::of(reviewed.verdict, &model());
    assert_eq!(review.approves(), approved);
    assert!(matches!(review, Review::Decided { decision } if decision.elapsed_ms.is_some()));
    let requests = server.requests.lock();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].starts_with("POST /systemone "));
    assert!(
      requests
        .iter()
        .all(|request| !request.to_lowercase().contains("authorization:"))
    );
    assert!(requests[1].contains("\"model\":\"jev-latest\""));
  }
}

#[tokio::test]
async fn decision_review_authentication_and_failures_close_with_reason() {
  for (status, body, failure) in [
    ("401 Unauthorized", "{}".to_owned(), ReviewFailure::KeyRefused),
    ("404 Not Found", "{}".to_owned(), ReviewFailure::NotSystemOneApi),
    (
      "500 Internal Server Error",
      "{}".to_owned(),
      ReviewFailure::NothingAnswers,
    ),
    ("200 OK", "not JSON".to_owned(), ReviewFailure::Unreadable),
  ] {
    let server = Server::new(status, body, Duration::ZERO, true).await;
    let reviewed = review(connection(&server.url, true), Some("test-review-key")).await;
    assert!(
      matches!(Review::of(reviewed.verdict, &model()), Review::Failed { failure: actual, .. } if actual == failure)
    );
    assert!(
      server
        .requests
        .lock()
        .iter()
        .all(|request| request.contains("Bearer test-review-key"))
    );
  }
}

#[tokio::test]
async fn decision_review_timeout_is_two_seconds() {
  let server = Server::new("200 OK", answer(1.0).to_string(), Duration::from_secs(3), true).await;
  let started = Instant::now();
  let reviewed = review(connection(&server.url, false), None).await;
  assert!(matches!(reviewed.verdict, Err(ReviewError::TimedOut)));
  assert!(started.elapsed() >= Duration::from_millis(1800));
  assert!(started.elapsed() < Duration::from_secs(3));
}

#[tokio::test]
async fn decision_review_gone_model_and_missing_key_send_no_inference() {
  let server = Server::new("200 OK", answer(1.0).to_string(), Duration::ZERO, false).await;
  let reviewed = review(connection(&server.url, false), None).await;
  assert!(matches!(
    reviewed.verdict,
    Err(ReviewError::Decision(ReviewFailure::ModelGone))
  ));
  assert_eq!(server.requests.lock().len(), 1);
  let reviewed = review(connection(&server.url, true), None).await;
  assert!(matches!(
    reviewed.verdict,
    Err(ReviewError::Decision(ReviewFailure::KeyRefused))
  ));
  assert_eq!(server.requests.lock().len(), 1);
}

#[tokio::test]
async fn decision_review_no_server_fails_closed() {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
  let url = format!("http://{}", listener.local_addr().expect("address"));
  drop(listener);
  let reviewed = review(connection(&url, false), None).await;
  // Windows can keep retrying a refused TCP connection until the request's deadline.
  assert!(
    matches!(
      reviewed.verdict,
      Err(ReviewError::TimedOut | ReviewError::Decision(ReviewFailure::NothingAnswers))
    ),
    "{reviewed:?}"
  );
}

#[tokio::test]
async fn decision_review_rejects_credential_echo_and_keeps_unknown_usage_unknown() {
  let mut body = answer(1.0);
  body["model"] = serde_json::json!("test-review-key");
  let server = Server::new("200 OK", body.to_string(), Duration::ZERO, true).await;
  let reviewed = review(connection(&server.url, true), Some("test-review-key")).await;
  assert!(matches!(reviewed.verdict, Err(ReviewError::Unparseable(_))));
  assert!(!format!("{reviewed:?}").contains("test-review-key"));
  let mut body = answer(1.0);
  body.as_object_mut().expect("object").remove("usage");
  let server = Server::new("200 OK", body.to_string(), Duration::ZERO, true).await;
  let reviewed = review(connection(&server.url, false), None).await;
  assert!(reviewed.verdict.is_ok());
  assert_eq!(reviewed.spend, ReviewSpend::Unknown);
}

#[tokio::test]
async fn decision_review_unreadable_answers_retain_usage_and_version_at_a_prefixed_url() {
  let mut body = answer(1.0);
  body["answers"]["verdict"]["probabilities"]["approve"] = serde_json::json!(1.5);
  let server = Server::new("200 OK", body.to_string(), Duration::ZERO, true).await;
  let reviewed = review(connection(&format!("{}/v1", server.url), false), None).await;
  assert!(matches!(reviewed.verdict, Err(ReviewError::Unparseable(_))));
  let cost = ReviewCost::of(&reviewed, &model()).expect("cost");
  assert_eq!(cost.model.as_str(), "jev-documentation-example");
  assert_eq!(cost.usage.expect("usage").input, 120);
  let requests = server.requests.lock();
  assert!(requests[0].starts_with("GET /v1/models "));
  assert!(requests[1].starts_with("POST /v1/systemone "));
}

#[tokio::test(flavor = "multi_thread")]
async fn decision_review_worker_records_usage_and_approves_manual_request() {
  let server = Server::new("200 OK", answer(0.8).to_string(), Duration::ZERO, true).await;
  let (project, connection) = configured_project(&server.url);
  let root = project.repo.path();
  let (scheduler, mut events) = support::scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  let run = support::wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  let db = ProjectDb::open(root).expect("open");
  let reviews = db.decision_reviewer_usage(id).expect("usage");
  assert_eq!(reviews.len(), 1);
  assert!(
    server
      .requests
      .lock()
      .iter()
      .all(|request| request.contains("Bearer project-decision-key"))
  );
  assert_eq!(reviews[0].connection, connection.id());
  assert_eq!(reviews[0].provider, "Local decision");
  assert_eq!(reviews[0].model.as_str(), "jev-documentation-example");
  assert_eq!(reviews[0].input_tokens, Some(120));
  let decided = db.decided_tool_approvals(id).expect("decided");
  assert_eq!(reviews[0].approval, decided[0].request.id);
  assert_eq!(reviews[0].invocation, decided[0].request.invocation);
  assert!(db.reviewer_usage(id).expect("language usage").is_empty());
  let agent: u64 = db
    .invocations(id)
    .expect("invocations")
    .iter()
    .map(|invocation| invocation.usage.counted_tokens().get())
    .sum();
  assert_eq!(run.used().get(), agent + 120);
  assert_eq!(
    db.tokens_used_since(Timestamp::from_unix_millis(0))
      .expect("daily budget")
      .get(),
    agent + 120
  );
}

#[tokio::test]
#[ignore = "live inference: set ORCHESTER_DECISION_REVIEW_URL and ORCHESTER_DECISION_REVIEW_MODEL"]
async fn decision_review_live_server_from_environment() {
  let url = std::env::var("ORCHESTER_DECISION_REVIEW_URL").expect("server URL");
  let model = ModelId::new(&std::env::var("ORCHESTER_DECISION_REVIEW_MODEL").expect("model")).expect("valid model");
  let mut connection = connection(&url, false);
  connection.record_models(vec![model.clone()]);
  let reviewer = DecisionReviewer::new(
    tokio::runtime::Handle::current(),
    connection,
    model,
    Probability::new(0.8).expect("threshold"),
    None,
  );
  let reviewed = tokio::task::spawn_blocking(move || reviewer.review(&input()))
    .await
    .expect("review");
  assert!(matches!(reviewed.verdict, Ok(ReviewVerdict::Decision(_))));
}

fn configured_project(url: &str) -> (support::TestProject, DecisionConnection) {
  use orchester_core::secret::{Environment, NewSecret, Secret, SecretKind, SecretName};
  let connection = connection(url, true);
  let secret = Secret::new(
    connection.key().expect("key id"),
    NewSecret {
      name: SecretName::new("DECISION_KEY").expect("secret name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  );
  let permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(
      AllowedCommands::new(vec![CommandPrefix::new("git").expect("prefix")]).expect("commands"),
    ),
    NetworkAccess::On,
  );
  let project = support::project(&support::Setup {
    provider: orchester_core::connection::AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions),
    secrets: vec![(secret, "project-decision-key")],
    ..support::Setup::new("Run `cargo build`.")
  });
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  db.create_decision_connection(&connection).expect("connection");
  db.save_auto_mode_settings(&AutoModeSettings {
    reviewer: Some(ReviewerChoice::decision(
      connection.id(),
      model(),
      Some(Probability::new(0.8).expect("threshold")),
    )),
  })
  .expect("settings");
  (project, connection)
}

#[tokio::test(flavor = "multi_thread")]
async fn decision_review_worker_asks_person_without_language_fallback() {
  for (status, body, failure) in [
    ("200 OK", answer(0.79).to_string(), None),
    ("401 Unauthorized", "{}".to_owned(), Some(ReviewFailure::KeyRefused)),
  ] {
    let server = Server::new(status, body, Duration::ZERO, true).await;
    let (project, _) = configured_project(&server.url);
    let root = project.repo.path();
    let (scheduler, mut events) = support::scheduler_with(root, project.vault.clone());
    let id = scheduler
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("run");
    let (request, review) = tokio::time::timeout(support::PATIENCE, async {
      loop {
        let db = ProjectDb::open(root).expect("open");
        if let Some(request) = db.pending_tool_approvals(id).expect("requests").first()
          && let Some(review) = db.tool_approval_review(id, &request.id).expect("review")
        {
          break (request.clone(), review);
        }
        events.recv().await.expect("event");
      }
    })
    .await
    .expect("request comes to person");
    assert!(!review.approves());
    match failure {
      Some(failure) => assert!(
        matches!(review, Review::Failed { failure: actual, decision: true, .. } if actual == failure),
        "a decision model's failure is marked as its: {review:?}"
      ),
      None => assert!(matches!(review, Review::Decided { .. })),
    }
    scheduler
      .decide_tool_approval(
        id,
        request.id,
        orchester_core::run::ApprovalAnswer::Deny,
        orchester_core::run::Answerer::Person,
      )
      .await
      .expect("person denies");
    support::wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
    let db = ProjectDb::open(root).expect("open");
    assert!(db.reviewer_usage(id).expect("language usage").is_empty());
    let records = db.decision_reviewer_usage(id).expect("decision usage");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].input_tokens, failure.is_none().then_some(120));
    assert_eq!(
      records[0].end,
      failure.map_or(
        orchester_core::run::review::ReviewEnd::Asked,
        orchester_core::run::review::ReviewEnd::Failed
      )
    );
  }
}
