//! Seeds end to end against `orchester-fake-codex`, with `orchester-test-command` as the service
//! (`support::services`) and its small JSON API (`support::fake_server`): once the service is ready, its seed requests
//! run in order, each filled with what the requests before it captured, and `start_service` answers the captured
//! values; a seed command runs without a shell. A failing request fails the start, naming the request, its status and
//! the start of its answer, and stops the service. A header's secret value reaches no answer and no log. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test service_seeds seed`.

mod support;

use std::{fs, path::Path};

use orchester_core::{
  id::{IdempotencyKey, RunId, SecretId},
  resource::{CommandArg, CommandProgram, FieldName, HttpMethod},
  run::RunStatus,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  service::{Capture, CapturePath, ReadyCheck, Seed, SeedHeader, SeedRequest, SeedRequests, Service, ServiceValue},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use self::support::{
  StepSpec,
  qa::{approve, gone},
  reported, scheduler, scheduler_with,
  services::{answered, publish, report_file, reported_pid, service, service_project, started_port},
  tools::TEST_COMMAND,
  wait_for,
};

/// The value of the project secret `ADMIN_KEY`: no answer or log of the run may show it.
const CANARY: &str = "canary-admin-key-5e1f9";

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

/// The service `web`, a stand-in server that reports its pid in `web.pid` and listens on its port, with `seed`.
fn seeded(root: &Path, seed: Seed) -> Service {
  let arguments = [
    "pid-file".to_owned(),
    report_file(root, "web.pid"),
    "listen".to_owned(),
    "{{port}}".to_owned(),
  ];
  service("web", &arguments, ReadyCheck::PortOpen, 20).with_seed(seed)
}

/// A seed of `requests`.
fn requests(requests: Vec<SeedRequest>) -> Seed {
  Seed::Requests(SeedRequests::new(requests).expect("captures named once"))
}

/// A request of `method` to `url` with `body`, `headers` and `captures` (`name ← path`).
fn request(
  method: HttpMethod,
  url: &str,
  body: Option<&str>,
  headers: Vec<SeedHeader>,
  captures: &[(&str, &str)],
) -> SeedRequest {
  let captures = captures
    .iter()
    .map(|(name, path)| {
      Capture::new(
        FieldName::new(name).expect("a name"),
        CapturePath::new(path).expect("a path"),
      )
    })
    .collect();
  SeedRequest::new(method, url, body.map(str::to_owned), headers, captures).expect("a request")
}

/// The header `X-Admin-Key`, sent as the project secret `ADMIN_KEY`.
fn admin_key() -> SeedHeader {
  SeedHeader::new(
    "X-Admin-Key",
    ServiceValue::Secret(SecretName::new("ADMIN_KEY").expect("a name")),
  )
  .expect("a header")
}

/// The project secret `ADMIN_KEY`, whose value is [`CANARY`].
fn admin_secret() -> Secret {
  Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("ADMIN_KEY").expect("a name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Staging,
    },
    Timestamp::now(),
  )
}

/// The JSON of a `start_service ok: …` answer.
fn started(answer: &str) -> serde_json::Value {
  started_port(answer);
  let json = answer.strip_prefix("start_service ok: ").expect("the service started");
  serde_json::from_str(json).expect("the answer is JSON")
}

/// Every line of the run's transcript.
fn transcript(root: &Path, run: RunId) -> Vec<String> {
  reported(&ProjectDb::open(root).expect("open"), run, "")
}

#[tokio::test(flavor = "multi_thread")]
async fn seed_requests_capture_values_for_later_requests_and_the_agent() {
  let project = service_project(implement("Start the service web."), Vec::new());
  let root = project.repo.path();
  let user = SeedHeader::new("X-User", ServiceValue::Plain("{{seed.user_id}}".to_owned())).expect("a header");
  let seed = requests(vec![
    request(
      HttpMethod::Post,
      "{{url}}/api/teams",
      Some(r#"{"name": "Blue", "port": "{{port}}"}"#),
      Vec::new(),
      &[("team_id", "$.team.id")],
    ),
    request(
      HttpMethod::Post,
      "{{url}}/api/teams/{{seed.team_id}}/users",
      Some(r#"{"name": "Ada"}"#),
      Vec::new(),
      &[("user_id", "$.users[0].id"), ("admin", "$.users[0].admin")],
    ),
    request(
      HttpMethod::Get,
      "{{url}}/api/session",
      None,
      vec![user],
      &[("token", "$.token"), ("expires", "$.expires")],
    ),
  ]);
  publish(root, &[seeded(root, seed)]);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = started(&answered(root, &mut events, id, "start_service").await);
  assert_eq!(
    answer["seed"],
    json!({"team_id": "team-7", "user_id": "user-3", "admin": "true", "token": "tok-9", "expires": "3600"}),
    "{answer}"
  );
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn seed_request_that_fails_fails_the_start_with_its_status_and_answer() {
  let project = service_project(implement("Start the service web."), Vec::new());
  let root = project.repo.path();
  let seed = requests(vec![
    request(
      HttpMethod::Post,
      "{{url}}/api/teams",
      Some(r#"{"name": "Blue"}"#),
      Vec::new(),
      &[("team_id", "$.team.id")],
    ),
    request(HttpMethod::Get, "{{url}}/api/fail", None, Vec::new(), &[]),
  ]);
  publish(root, &[seeded(root, seed)]);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = answered(root, &mut events, id, "start_service").await;
  assert!(
    answer
      .starts_with("start_service error: the service `web` ran its seed, but seed request 2 (GET http://127.0.0.1:"),
    "{answer}"
  );
  assert!(
    answer.contains(r#"/api/fail) answered 500 Internal Server Error: `{"error":"database is locked"}xxx"#),
    "{answer}"
  );
  // The excerpt is capped: the answer's 2000 `x` are cut.
  assert!(answer.contains("x…`"), "{answer}");
  assert!(!answer.contains(&"x".repeat(1000)), "{answer}");
  assert!(answer.contains("it was stopped with everything it started"), "{answer}");
  gone(&reported_pid(root, "web.pid").await, "the service whose seed failed").await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn seed_secret_header_reaches_no_answer_and_no_log() {
  let project = service_project(
    implement("Start the service web. Then look at the service."),
    vec![(admin_secret(), CANARY)],
  );
  let root = project.repo.path();
  // The server prints every header it gets, and `/api/key` answers the header's value, which the seed captures.
  let seed = requests(vec![request(
    HttpMethod::Get,
    "{{url}}/api/key",
    None,
    vec![admin_key()],
    &[("key", "$.key")],
  )]);
  publish(root, &[seeded(root, seed)]);
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = started(&answered(root, &mut events, id, "start_service").await);
  assert_eq!(answer["seed"], json!({"key": "<redacted>"}), "{answer}");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let lines = transcript(root, id);
  assert!(
    lines
      .iter()
      .any(|line| line.ends_with("out| header x-admin-key: <redacted>")),
    "the service's log shows the header it got, redacted: {lines:?}"
  );
  assert!(
    lines.iter().all(|line| !line.contains(CANARY)),
    "the secret's value reached the transcript"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn seed_secret_header_is_redacted_in_a_failed_answer() {
  let project = service_project(implement("Start the service web."), vec![(admin_secret(), CANARY)]);
  let root = project.repo.path();
  // `/api/echo` fails with every header it got in its answer.
  let seed = requests(vec![request(
    HttpMethod::Post,
    "{{url}}/api/echo",
    Some("{}"),
    vec![admin_key()],
    &[],
  )]);
  publish(root, &[seeded(root, seed)]);
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = answered(root, &mut events, id, "start_service").await;
  assert!(answer.contains("seed request 1 (POST http://127.0.0.1:"), "{answer}");
  assert!(answer.contains("x-admin-key=<redacted>"), "{answer}");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let lines = transcript(root, id);
  assert!(
    lines.iter().all(|line| !line.contains(CANARY)),
    "the secret's value reached the transcript"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn seed_command_runs_once_the_service_is_ready() {
  let project = service_project(implement("Start the service web."), Vec::new());
  let root = project.repo.path();
  let seeded_file = report_file(root, "seeded");
  let program = CommandProgram::new(TEST_COMMAND).expect("the test program");
  let arguments = ["connect", "{{port}}", "write", &seeded_file, "seeded {{port}}"]
    .iter()
    .map(|argument| CommandArg::new(argument).expect("an argument"))
    .collect();
  publish(root, &[seeded(root, Seed::Command { program, arguments })]);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = started(&answered(root, &mut events, id, "start_service").await);
  assert!(answer.get("seed").is_none(), "a command captures nothing: {answer}");
  let port = answer["port"].as_u64().expect("a port");
  // The command ran after the service was ready: it reached the service's port.
  assert_eq!(
    fs::read_to_string(&seeded_file).expect("seeded"),
    format!("seeded {port}")
  );
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn seed_command_that_fails_fails_the_start() {
  let project = service_project(implement("Start the service web."), Vec::new());
  let root = project.repo.path();
  let program = CommandProgram::new(TEST_COMMAND).expect("the test program");
  let arguments = ["stderr", "no such table: teams", "exit", "4"]
    .iter()
    .map(|argument| CommandArg::new(argument).expect("an argument"))
    .collect();
  publish(root, &[seeded(root, Seed::Command { program, arguments })]);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = answered(root, &mut events, id, "start_service").await;
  assert!(
    answer.starts_with("start_service error: the service `web` ran its seed command"),
    "{answer}"
  );
  assert!(answer.contains("which exited with"), "{answer}");
  gone(&reported_pid(root, "web.pid").await, "the service whose seed failed").await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let lines = transcript(root, id);
  assert!(
    lines.iter().any(|line| line.ends_with("err| no such table: teams")),
    "{lines:?}"
  );
}
