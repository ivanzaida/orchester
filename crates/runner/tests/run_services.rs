//! The task page's Services tab's requests to the runner end to end (ORC-265), against `orchester-fake-codex` with
//! `orchester-test-command` as the services (`support::services`): the run's services are listed with their state,
//! port and URL while its step holds them; a service's log lines come with their secret's value redacted, later lines
//! after the last one read; the person's Stop stops it and frees its port, and Restart starts it again on a new port,
//! started by the person, but never a definition published again since its approval; a crash shows with its exit code
//! and the service that needs it as stopped for it; Retry after Docker was not available starts it again. Needs
//! `git`. Run with `cargo test -p orchester-runner --features test-agent --test run_services run_services`.

mod support;

use std::{collections::BTreeMap, fs, path::Path, time::Duration};

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::{IdempotencyKey, RunId, SecretId},
  resource::{CommandEnvironment, CommandProgram, Draft, ResourceBody, VariableName},
  run::RunStatus,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  service::{
    LogPattern, Readiness, ReadyCheck, ReadyTimeout, RunData, Service, ServiceName, ServiceStartFailure, ServiceValue,
    ServiceVariables, StartFailureCause,
  },
  store::{ActivityStore, ResourceStore, SettingsStore},
  time::Timestamp,
};
use orchester_ipc::{
  Rejection,
  services::{RunService, RunServiceState, ServiceCommand, ServiceReply, ServiceStarter},
};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;

use self::support::{
  PATIENCE, StepSpec,
  qa::{approve, calls},
  scheduler_with,
  services::{accepts, answered, closed, publish, report_file, service, service_project, started_port},
  tools::TEST_COMMAND,
  wait_for,
};

const TOKEN: &str = "panel-token-value-7c2e";

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

fn token_secret() -> Secret {
  Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("SERVICE_TOKEN").expect("a name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Staging,
    },
    Timestamp::now(),
  )
}

fn token_variable() -> ServiceVariables {
  ServiceVariables::new(BTreeMap::from([(
    VariableName::new("API_TOKEN").expect("a name"),
    ServiceValue::Secret(SecretName::new("SERVICE_TOKEN").expect("a name")),
  )]))
  .expect("variables")
}

async fn listed(scheduler: &Scheduler, run: RunId) -> Vec<RunService> {
  match scheduler.services(ServiceCommand::List { run }).await {
    Ok(ServiceReply::Listed { services, .. }) => services,
    other => panic!("the run's services: {other:?}"),
  }
}

/// The run's services once `done` holds for them.
async fn listed_until(
  scheduler: &Scheduler,
  run: RunId,
  what: &str,
  done: impl Fn(&[RunService]) -> bool,
) -> Vec<RunService> {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let services = listed(scheduler, run).await;
      if done(&services) {
        return services;
      }
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  })
  .await
  .unwrap_or_else(|_| panic!("{what} in time"))
}

async fn log(scheduler: &Scheduler, run: RunId, since: u64) -> (Vec<String>, u64) {
  let command = ServiceCommand::Log {
    run,
    service: "web".to_owned(),
    since,
    tail: 100,
  };
  match scheduler.services(command).await {
    Ok(ServiceReply::Log { lines, last }) => (lines.into_iter().map(|line| line.text).collect(), last),
    other => panic!("the service's log: {other:?}"),
  }
}

/// Publishes `service` again, as a person does who changes it on the project's Services page.
fn republish(root: &Path, service: &Service) {
  let mut db = ProjectDb::open(root).expect("open");
  let resource = db
    .resources(Service::KIND)
    .expect("services")
    .into_iter()
    .find(|resource| resource.slug().as_str() == service.name().as_str())
    .expect("the service is published");
  db.update_draft::<Service>(resource.id(), |_, latest| {
    Ok(Draft::new(
      service.clone(),
      latest.map(|version| version.number()),
      Timestamp::now(),
    ))
  })
  .expect("a draft");
  db.publish_draft::<Service>(resource.id(), Timestamp::now())
    .expect("published again");
}

fn hold(root: &Path) -> std::path::PathBuf {
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  hold
}

#[tokio::test(flavor = "multi_thread")]
async fn run_services_list_log_stop_and_restart_through_the_runner() {
  let project = service_project(
    implement("Start the service web. Hold the services."),
    vec![(token_secret(), TOKEN)],
  );
  let root = project.repo.path();
  let then = ["env", "API_TOKEN", "say", "booted", "listen", "{{port}}"];
  let mut arguments = vec!["pid-file".to_owned(), report_file(root, "web.pid")];
  arguments.extend(then.iter().map(|argument| (*argument).to_owned()));
  let ready = ReadyCheck::LogLine(LogPattern::new("^booted$").expect("a pattern"));
  publish(
    root,
    &[service("web", &arguments, ready, 20).with_variables(token_variable())],
  );
  let hold = hold(root);
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let port = started_port(&answered(root, &mut events, id, "start_service").await);

  let services = listed(&scheduler, id).await;
  assert_eq!(services.len(), 1, "{services:?}");
  let web = &services[0];
  assert_eq!(
    (web.name.as_str(), web.state, web.port, web.started_by),
    ("web", RunServiceState::Ready, port, ServiceStarter::Agent)
  );
  assert_eq!(web.url, format!("http://127.0.0.1:{port}"));
  assert!(web.ended_at.is_none(), "{web:?}");

  let (lines, last) = log(&scheduler, id, 0).await;
  assert!(lines.iter().any(|line| line == "API_TOKEN=<redacted>"), "{lines:?}");
  assert!(
    lines.iter().all(|line| !line.contains(TOKEN)),
    "the secret's value reached the log"
  );
  let (later, _) = log(&scheduler, id, last).await;
  assert!(later.is_empty(), "nothing after the last line read: {later:?}");

  let stopped = scheduler
    .services(ServiceCommand::Stop {
      run: id,
      service: "web".to_owned(),
    })
    .await;
  assert_eq!(stopped, Ok(ServiceReply::Done));
  closed(port, "the service the person stopped").await;
  let services = listed(&scheduler, id).await;
  assert_eq!(services[0].state, RunServiceState::Stopped);
  assert!(services[0].ended_at.is_some(), "{services:?}");

  let restarted = scheduler
    .services(ServiceCommand::Restart {
      run: id,
      service: "web".to_owned(),
    })
    .await;
  assert_eq!(restarted, Ok(ServiceReply::Done));
  let services = listed(&scheduler, id).await;
  assert_eq!(services.len(), 1, "{services:?}");
  let again = &services[0];
  assert_eq!(
    (again.state, again.started_by),
    (RunServiceState::Ready, ServiceStarter::Person)
  );
  assert!(accepts(again.port), "the restarted service listens");

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  closed(again.port, "the restarted service once its step ended").await;
  assert!(
    listed(&scheduler, id).await.is_empty(),
    "a run whose steps ended has none"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn run_services_show_a_crash_and_what_it_stopped() {
  let project = service_project(implement("Start the service web. Hold the services."), Vec::new());
  let root = project.repo.path();
  let api = ["say", "ready", "sleep", "2", "stderr", "giving up", "exit", "3"].map(str::to_owned);
  let api = service(
    "api",
    &api,
    ReadyCheck::LogLine(LogPattern::new("^ready$").expect("a pattern")),
    20,
  );
  let web = service(
    "web",
    &["listen".to_owned(), "{{port}}".to_owned()],
    ReadyCheck::PortOpen,
    20,
  )
  .with_needs(vec![ServiceName::new("api").expect("a name")])
  .expect("needs");
  publish(root, &[api, web]);
  let hold = hold(root);
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  started_port(&answered(root, &mut events, id, "start_service").await);

  let services = listed_until(&scheduler, id, "the crash", |services| {
    services
      .iter()
      .any(|service| matches!(service.state, RunServiceState::Crashed { .. }))
  })
  .await;
  let names: Vec<&str> = services.iter().map(|service| service.name.as_str()).collect();
  assert_eq!(names, ["api", "web"], "in the order they started");
  assert_eq!(services[0].state, RunServiceState::Crashed { code: Some(3) });
  assert!(services[0].ended_at.is_some());
  assert_eq!(services[1].state, RunServiceState::Stopped);
  assert_eq!(services[1].stopped_for.as_deref(), Some("api"));

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

/// A person's Restart goes on while the agent's start of another service still waits for it to be ready.
#[tokio::test(flavor = "multi_thread")]
async fn run_services_restart_does_not_wait_for_the_agents_start_under_way() {
  let project = service_project(
    implement("Start the service api. Then start the service slow. Hold the services."),
    Vec::new(),
  );
  let root = project.repo.path();
  let api = service(
    "api",
    &["listen".to_owned(), "{{port}}".to_owned()],
    ReadyCheck::PortOpen,
    20,
  );
  let never = ["say", "warming up", "sleep", "600"].map(str::to_owned);
  let slow = service("slow", &never, ReadyCheck::PortOpen, 20);
  publish(root, &[api, slow]);
  let hold = hold(root);
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let first = started_port(&answered(root, &mut events, id, "start_service").await);
  approve(&scheduler, root, &mut events, id).await;
  listed_until(&scheduler, id, "slow starting", |services| {
    services
      .iter()
      .any(|service| service.name == "slow" && service.state == RunServiceState::Starting)
  })
  .await;

  let restart = ServiceCommand::Restart {
    run: id,
    service: "api".to_owned(),
  };
  let restarted = tokio::time::timeout(Duration::from_secs(10), scheduler.services(restart))
    .await
    .expect("the restart does not wait for slow's start");
  assert_eq!(restarted, Ok(ServiceReply::Done));
  let services = listed(&scheduler, id).await;
  let api = services
    .iter()
    .find(|service| service.name == "api")
    .expect("api is listed");
  assert_eq!(
    (api.state, api.started_by),
    (RunServiceState::Ready, ServiceStarter::Person)
  );
  assert_ne!(api.port, first, "the restarted service listens on a port of its own");
  assert!(accepts(api.port));

  // Stop on a service that still starts gives its start up: the agent's call answers, and the step goes on.
  let stop = ServiceCommand::Stop {
    run: id,
    service: "slow".to_owned(),
  };
  assert_eq!(scheduler.services(stop).await, Ok(ServiceReply::Done));
  listed_until(&scheduler, id, "slow stopped", |services| {
    services
      .iter()
      .any(|service| service.name == "slow" && service.state == RunServiceState::Stopped)
  })
  .await;
  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answers = calls(root, id);
  assert!(
    answers
      .iter()
      .any(|call| call.starts_with("start_service error: the service `slow` was stopped before it was ready")),
    "{answers:?}"
  );
}

/// A person's Restart runs only what an approved start ran: once the service is published again with another command,
/// the Restart is refused before anything stops, and the service the agent started keeps running.
#[tokio::test(flavor = "multi_thread")]
async fn run_services_restart_refuses_a_definition_published_again_since_its_approval() {
  let project = service_project(implement("Start the service web. Hold the services."), Vec::new());
  let root = project.repo.path();
  let listen = ["listen".to_owned(), "{{port}}".to_owned()];
  publish(root, &[service("web", &listen, ReadyCheck::PortOpen, 20)]);
  let hold = hold(root);
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let port = started_port(&answered(root, &mut events, id, "start_service").await);

  let changed = ["say", "changed", "listen", "{{port}}"].map(str::to_owned);
  republish(root, &service("web", &changed, ReadyCheck::PortOpen, 20));
  let restart = ServiceCommand::Restart {
    run: id,
    service: "web".to_owned(),
  };
  match scheduler.services(restart).await {
    Err(Rejection::Failed { detail }) => assert!(
      detail.contains("published differently from when this step's agent started it"),
      "{detail}"
    ),
    other => panic!("a definition nobody approved is refused: {other:?}"),
  }
  let services = listed(&scheduler, id).await;
  assert_eq!(services.len(), 1, "{services:?}");
  assert_eq!(
    (services[0].state, services[0].port, services[0].started_by),
    (RunServiceState::Ready, port, ServiceStarter::Agent),
    "the refused Restart stopped nothing"
  );
  assert!(accepts(port), "the agent's service still listens");

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

/// The service failed starts noted in the run's conversation, oldest first.
fn start_failures(root: &Path, run: RunId) -> Vec<ServiceStartFailure> {
  ProjectDb::open(root)
    .expect("open")
    .activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem {
        problem: RunnerProblem::ServiceStartFailed(failure),
      } => Some(*failure),
      _ => None,
    })
    .collect()
}

/// Retry on the no-Docker card (`oXVuM` 10a): a start that failed before its program started, Docker not answering,
/// was never a running service, yet the person's Restart reaches the runner and starts it again as the agent's start
/// ran it, which asks Docker again: with Docker still not answering, the refusal is Docker's and the conversation
/// notes the failed start again.
#[tokio::test(flavor = "multi_thread")]
async fn run_services_retry_after_docker_was_not_available_starts_the_service_again() {
  let project = service_project(implement("Start the service db. Hold the services."), Vec::new());
  let root = project.repo.path();
  // A `docker` first on the project's PATH whose engine never names a version.
  let bin = root.join(".orchester").join("stand-in-bin");
  fs::create_dir_all(&bin).expect("bin");
  let docker = bin.join(if cfg!(windows) { "docker.exe" } else { "docker" });
  fs::copy(TEST_COMMAND, &docker).expect("a stand-in docker");
  let inherited = std::env::var_os("PATH").unwrap_or_default();
  let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&inherited))).expect("a PATH");
  let environment = CommandEnvironment::new(BTreeMap::from([(
    VariableName::new("PATH").expect("a name"),
    path.into_string().expect("a Unicode PATH"),
  )]))
  .expect("an environment");
  ProjectDb::open(root)
    .expect("open")
    .save_project_environment(&environment)
    .expect("saved");
  let readiness = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(20).expect("in range"));
  let database = Service::new(
    ServiceName::new("db").expect("a name"),
    CommandProgram::new("redis:7-alpine").expect("an image"),
    readiness,
  )
  .with_run_data(RunData::DatabaseContainer);
  publish(root, &[database]);
  let hold = hold(root);
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = answered(root, &mut events, id, "start_service").await;
  assert!(answer.contains("Docker is not available"), "{answer}");
  assert_eq!(start_failures(root, id).len(), 1, "the failed start is noted");
  assert!(listed(&scheduler, id).await.is_empty(), "no service ran");

  let retry = ServiceCommand::Restart {
    run: id,
    service: "db".to_owned(),
  };
  match scheduler.services(retry).await {
    Err(Rejection::Failed { detail }) => assert!(
      detail.contains("the service `db` cannot be started: Docker is not available"),
      "the Retry reached the start, which asked Docker again: {detail}"
    ),
    other => panic!("Docker still does not answer: {other:?}"),
  }
  let failures = start_failures(root, id);
  assert_eq!(failures.len(), 2, "the Retry's failed start is noted too: {failures:?}");
  assert!(
    matches!(failures[1].cause, StartFailureCause::DockerNotAnswering { .. }),
    "{failures:?}"
  );

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}
