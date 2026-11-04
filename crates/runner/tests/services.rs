//! `start_service` and `stop_service` end to end against `orchester-fake-codex`, with `orchester-test-command` as the
//! service (`support::services`): the runner starts a published service on a free loopback port of the run, fills its
//! placeholders and variables, reads its secret from the keychain, and answers once it is ready by its port, an HTTP
//! answer or a log line. One not ready in time is stopped with everything it started. A service is stopped when its
//! step ends, when its run pauses and while its turn waits for a person; `stop_service` never asks. Two runs get
//! different ports. Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test services
//! service`.

mod support;

use std::{collections::BTreeMap, fs, num::NonZeroU32, path::Path};

use orchester_core::{
  id::{IdempotencyKey, SecretId},
  resource::VariableName,
  run::{PauseReason, RunStatus},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  service::{HttpPath, LogPattern, ReadyCheck, ServiceValue, ServiceVariables},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{
  StepSpec, is_running,
  qa::{approve, calls, gone},
  queue_another_task, scheduler, scheduler_with,
  services::{accepts, answered, closed, publish, report_file, reported_pid, service, service_project, started_port},
  wait_for,
};

const TOKEN: &str = "service-token-value-4b1d";

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

/// The arguments of a stand-in server that reports its pid in `pid`, then does `then`.
fn server(root: &Path, pid: &str, then: &[&str]) -> Vec<String> {
  let mut arguments = vec!["pid-file".to_owned(), report_file(root, pid)];
  arguments.extend(then.iter().map(|argument| (*argument).to_owned()));
  arguments
}

#[tokio::test(flavor = "multi_thread")]
async fn service_ready_by_port_runs_for_its_step_and_stops_when_it_ends() {
  let project = service_project(implement("Start the service web. Hold the services."), Vec::new());
  let root = project.repo.path();
  let arguments = server(root, "web.pid", &["listen", "{{port}}"]);
  publish(root, &[service("web", &arguments, ReadyCheck::PortOpen, 20)]);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let port = started_port(&answered(root, &mut events, id, "start_service").await);
  let pid = reported_pid(root, "web.pid").await;
  assert!(is_running(&pid), "the service runs while its step works");
  assert!(accepts(port), "the service listens on the port it was given");

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  gone(&pid, "the service after its step ended").await;
  closed(port, "the service's port once it stopped").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn service_ready_by_http_and_stop_service_never_asks() {
  let project = service_project(implement("Start the service web. Then stop the service."), Vec::new());
  let root = project.repo.path();
  // The server listens only after a second, and `/moved` answers 302: the start waits for that answer.
  let arguments = server(root, "web.pid", &["sleep", "1", "listen", "{{port}}"]);
  let ready = ReadyCheck::Http(HttpPath::new("/moved").expect("a path"));
  publish(root, &[service("web", &arguments, ready, 20)]);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answers = calls(root, id);
  assert!(
    answers.iter().any(|call| call.starts_with("start_service ok: ")),
    "{answers:?}"
  );
  assert!(
    answers
      .iter()
      .any(|call| call.starts_with("stop_service ok: the service `web` was stopped")),
    "{answers:?}"
  );
  let asked = ProjectDb::open(root)
    .expect("open")
    .decided_tool_approvals(id)
    .expect("decided");
  assert_eq!(asked.len(), 1, "only the start asked");
  assert_eq!(asked[0].request.action, "start_service");
  gone(&reported_pid(root, "web.pid").await, "the service stop_service stopped").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn service_ready_by_log_line_gets_its_variables_and_secret() {
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("SERVICE_TOKEN").expect("a name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Staging,
    },
    Timestamp::now(),
  );
  let project = service_project(implement("Start the service web."), vec![(secret, TOKEN)]);
  let root = project.repo.path();
  let (greeting, token) = (report_file(root, "greeting"), report_file(root, "token"));
  let then = [
    "env-file",
    &greeting,
    "GREETING",
    "env-file",
    &token,
    "API_TOKEN",
    "say",
    "booted on {{port}}",
    "sleep",
    "600",
  ];
  let variables = ServiceVariables::new(BTreeMap::from([
    (
      VariableName::new("GREETING").expect("a name"),
      ServiceValue::Plain("hello from {{port}}".to_owned()),
    ),
    (
      VariableName::new("API_TOKEN").expect("a name"),
      ServiceValue::Secret(SecretName::new("SERVICE_TOKEN").expect("a name")),
    ),
  ]))
  .expect("variables");
  let ready = ReadyCheck::LogLine(LogPattern::new(r"^booted on \d+$").expect("a pattern"));
  publish(
    root,
    &[service("web", &server(root, "web.pid", &then), ready, 20).with_variables(variables)],
  );
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let port = started_port(&answered(root, &mut events, id, "start_service").await);
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(
    fs::read_to_string(&greeting).expect("greeting"),
    format!("hello from {port}")
  );
  assert_eq!(fs::read_to_string(&token).expect("token"), TOKEN);
  gone(&reported_pid(root, "web.pid").await, "the service after its step ended").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn service_not_ready_in_time_is_stopped_with_everything_it_started() {
  let project = service_project(implement("Start the service web."), Vec::new());
  let root = project.repo.path();
  let child = report_file(root, "child.pid");
  let arguments = server(root, "web.pid", &["spawn-sleeper-into", &child, "sleep", "600"]);
  publish(root, &[service("web", &arguments, ReadyCheck::PortOpen, 2)]);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = answered(root, &mut events, id, "start_service").await;
  assert!(answer.starts_with("start_service error: "), "{answer}");
  assert!(answer.contains("was not ready within 2 s"), "{answer}");
  assert!(answer.contains("stopped with everything it started"), "{answer}");
  gone(&reported_pid(root, "web.pid").await, "the service that was not ready").await;
  gone(
    &reported_pid(root, "child.pid").await,
    "what the service that was not ready started",
  )
  .await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn service_stops_while_its_turn_waits_for_a_person() {
  let project = service_project(implement("Start the service web. Then read outside."), Vec::new());
  let root = project.repo.path();
  let deps = root.join("deps-a");
  fs::create_dir_all(&deps).expect("deps");
  fs::write(deps.join("notes.txt"), "outside line\n").expect("notes");
  let arguments = server(root, "web.pid", &["listen", "{{port}}"]);
  publish(root, &[service("web", &arguments, ReadyCheck::PortOpen, 20)]);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let port = started_port(&answered(root, &mut events, id, "start_service").await);
  let pid = reported_pid(root, "web.pid").await;
  // The read outside the worktree asks: while the person decides, the service is stopped.
  gone(&pid, "the service while its turn waits for a person").await;
  assert!(!accepts(port), "nothing listens on its port meanwhile");
  approve(&scheduler, root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn service_stops_when_its_run_pauses() {
  let steps = vec![
    StepSpec::new(
      "implement",
      "Implement",
      "Start the service web. Hold the services.",
      &[],
    ),
    StepSpec::new("review", "Review", "Look at it.", &["implement"]),
  ];
  let project = service_project(steps, Vec::new());
  let root = project.repo.path();
  let arguments = server(root, "web.pid", &["listen", "{{port}}"]);
  publish(root, &[service("web", &arguments, ReadyCheck::PortOpen, 20)]);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  started_port(&answered(root, &mut events, id, "start_service").await);
  let pid = reported_pid(root, "web.pid").await;
  scheduler.pause_run(id).await.expect("the pause is requested");
  fs::remove_file(&hold).expect("release");
  let paused = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::Requested));
  gone(&pid, "the service of the paused run").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn service_two_runs_get_different_ports() {
  let project = service_project(implement("Start the service web. Hold the services."), Vec::new());
  let root = project.repo.path();
  publish(
    root,
    &[service(
      "web",
      &["listen".to_owned(), "{{port}}".to_owned()],
      ReadyCheck::PortOpen,
      20,
    )],
  );
  let second_task = queue_another_task(root);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .set_agent_slots(NonZeroU32::new(2).expect("two"))
    .await
    .expect("two agents at once");
  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("first run");
  approve(&scheduler, root, &mut events, first).await;
  let first_port = started_port(&answered(root, &mut events, first, "start_service").await);
  let second = scheduler
    .start_run(IdempotencyKey::generate(), second_task)
    .await
    .expect("second run");
  approve(&scheduler, root, &mut events, second).await;
  let second_port = started_port(&answered(root, &mut events, second, "start_service").await);
  assert_ne!(first_port, second_port, "each run's service has a port of its own");
  assert!(
    accepts(first_port) && accepts(second_port),
    "both services listen at once"
  );
  fs::remove_file(&hold).expect("release");
  for run in [first, second] {
    let run = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
    assert_eq!(run.status(), &RunStatus::Succeeded);
  }
  closed(first_port, "the first run's service after its step ended").await;
  closed(second_port, "the second run's service after its step ended").await;
}
