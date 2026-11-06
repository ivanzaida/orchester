//! A service's per-run data end to end against `orchester-fake-codex`: a run folder is made in the run's storage
//! before the service starts, reaches it as `{{data}}`, and goes when the run ends; a runner that starts removes the
//! data folders runs left. A database container needs Docker: without it the start fails at once with "Docker is not
//! available", naming what was checked. With Docker the container starts per run on the service's port and is
//! removed when the run ends, and a container a runner left is removed when the next one starts; these tests say
//! that they were skipped, and why, when this computer has no Docker that answers. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test service_data service_data`.

mod support;

use std::{
  collections::BTreeMap,
  ffi::OsString,
  fs,
  path::PathBuf,
  time::{Duration, Instant},
};

use orchester_core::{
  command::CommandRunner,
  id::{IdempotencyKey, RunId},
  resource::{CommandEnvironment, CommandProgram, VariableName},
  run::RunStatus,
  service::{
    CONTAINER_PROJECT_LABEL, CONTAINER_RUN_LABEL, ContainerName, LogPattern, Readiness, ReadyCheck, ReadyTimeout,
    RunData, RunFolder, Service, ServiceName, ServiceValue, ServiceVariables, container_notes_folder, run_data_folder,
  },
  store::SettingsStore,
};
use orchester_exec::{ProcessRunner, docker::Docker, search_path::search_path};
use orchester_store::ProjectDb;

use self::support::{
  PATIENCE, StepSpec,
  qa::approve,
  scheduler,
  services::{accepts, answered, publish, report_file, service, service_project, started_port},
  tools::TEST_COMMAND,
  wait_for,
};

/// A small image whose server says when it is ready.
const IMAGE: &str = "redis:7-alpine";

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

/// Waits until `done` holds, checking every 100 ms.
async fn eventually(what: &str, done: impl Fn() -> bool) {
  let waited = tokio::time::timeout(PATIENCE, async {
    while !done() {
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  })
  .await;
  assert!(waited.is_ok(), "{what}");
}

/// Docker on this computer when its engine answers and has [`IMAGE`]; `None`, saying why, when not.
#[allow(
  clippy::print_stderr,
  reason = "a test skipped for want of Docker says so, as the task asks of these tests"
)]
fn docker_with_image(test: &str) -> Option<Docker> {
  let docker = match Docker::ready(search_path().as_deref()) {
    Ok(docker) => docker,
    Err(error) => {
      eprintln!("{test}: skipped, this computer has no Docker that answers: {error}");
      return None;
    }
  };
  if let Err(error) = docker.exposed_port(IMAGE) {
    eprintln!("{test}: skipped, Docker cannot get the image {IMAGE}: {error}");
    return None;
  }
  Some(docker)
}

/// `db`, the [`IMAGE`] container, ready once its server says so.
fn database() -> Service {
  let ready = ReadyCheck::LogLine(LogPattern::new("Ready to accept connections").expect("a pattern"));
  let readiness = Readiness::new(ready, ReadyTimeout::from_secs(60).expect("in range"));
  Service::new(
    ServiceName::new("db").expect("a name"),
    CommandProgram::new(IMAGE).expect("an image"),
    readiness,
  )
  .with_run_data(RunData::DatabaseContainer)
}

#[tokio::test(flavor = "multi_thread")]
async fn service_data_run_folder_is_made_before_the_start_and_goes_with_the_run() {
  let project = service_project(implement("Start the service web. Hold the services."), Vec::new());
  let root = project.repo.path();
  let reported = report_file(root, "data-dir");
  // The stand-in writes into `{{data}}` first: the folder is there before it starts.
  let arguments: Vec<String> = [
    "write",
    "{{data}}/started",
    "yes",
    "env-file",
    &reported,
    "DATA_DIR",
    "listen",
    "{{port}}",
  ]
  .into_iter()
  .map(str::to_owned)
  .collect();
  let variables = ServiceVariables::new(BTreeMap::from([(
    VariableName::new("DATA_DIR").expect("a name"),
    ServiceValue::Plain("{{data}}".to_owned()),
  )]))
  .expect("variables");
  let web = service("web", &arguments, ReadyCheck::PortOpen, 20)
    .with_variables(variables)
    .with_run_data(RunData::RunFolder(RunFolder::new("team/web").expect("a folder")));
  publish(root, &[web]);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  started_port(&answered(root, &mut events, id, "start_service").await);
  let folder = run_data_folder(root, id).join("team").join("web");
  assert_eq!(fs::read_to_string(folder.join("started")).expect("written"), "yes");
  assert_eq!(
    PathBuf::from(fs::read_to_string(&reported).expect("reported")),
    folder,
    "{{{{data}}}} is the run folder"
  );

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let data = run_data_folder(root, id);
  eventually("the run's data folder goes when the run ends", || !data.exists()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn service_data_left_by_a_runner_that_died_goes_when_the_next_starts() {
  let project = service_project(implement("Finish."), Vec::new());
  let root = project.repo.path();
  let left = run_data_folder(root, RunId::generate()).join("web");
  fs::create_dir_all(&left).expect("a run folder left");
  fs::write(left.join("table.db"), "rows").expect("a file");
  let (_scheduler, _events) = scheduler(root);
  eventually(
    "a data folder of a run that is gone goes when the runner starts",
    || !left.exists(),
  )
  .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn service_data_container_without_docker_fails_at_once() {
  let project = service_project(implement("Start the service db."), Vec::new());
  let root = project.repo.path();
  // A `docker` first on the project's PATH whose engine never names a version: the stand-in only says it does not
  // know `version`.
  let bin = root.join(".orchester").join("stand-in-bin");
  fs::create_dir_all(&bin).expect("bin");
  let docker = bin.join(if cfg!(windows) { "docker.exe" } else { "docker" });
  fs::copy(TEST_COMMAND, &docker).expect("a stand-in docker");
  let inherited = std::env::var_os("PATH").unwrap_or_default();
  let path: OsString =
    std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(&inherited))).expect("a PATH");
  let environment = CommandEnvironment::new(BTreeMap::from([(
    VariableName::new("PATH").expect("a name"),
    path.into_string().expect("a Unicode PATH"),
  )]))
  .expect("an environment");
  ProjectDb::open(root)
    .expect("open")
    .save_project_environment(&environment)
    .expect("saved");
  publish(root, &[database()]);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let asked = Instant::now();
  let answer = answered(root, &mut events, id, "start_service").await;
  assert!(asked.elapsed() < Duration::from_secs(15), "it fails at once");
  assert!(answer.starts_with("start_service error: "), "{answer}");
  assert!(
    answer.contains("the service `db` cannot be started: Docker is not available: the Docker engine did not answer `"),
    "{answer}"
  );
  assert!(
    answer.contains(&docker.display().to_string()),
    "it names the docker it asked: {answer}"
  );
  assert!(
    answer.contains("unknown action version"),
    "it says what docker answered: {answer}"
  );
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert!(!container_notes_folder(root, id).exists(), "no container was started");
}

#[tokio::test(flavor = "multi_thread")]
async fn service_data_container_starts_per_run_and_goes_when_it_ends() {
  let Some(docker) = docker_with_image("service_data_container_starts_per_run_and_goes_when_it_ends") else {
    return;
  };
  let project = service_project(implement("Start the service db. Hold the services."), Vec::new());
  let root = project.repo.path();
  publish(root, &[database()]);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let port = started_port(&answered(root, &mut events, id, "start_service").await);
  assert!(accepts(port), "the container listens on the service's port");
  let name = ContainerName::of(id, &ServiceName::new("db").expect("a name"), port);
  let of_run = || docker.labelled(CONTAINER_RUN_LABEL, &id.to_string()).expect("listed");
  assert_eq!(of_run(), [name.as_str().to_owned()]);
  assert!(
    container_notes_folder(root, id).join(name.as_str()).is_file(),
    "the run notes it"
  );

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  eventually("the run's container is removed when it ends", || of_run().is_empty()).await;
  let note = container_notes_folder(root, id).join(name.as_str());
  eventually("its note goes with it", || !note.exists()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn service_data_container_left_by_a_runner_that_died_goes_when_the_next_starts() {
  let Some(docker) = docker_with_image("service_data_container_left_by_a_runner_that_died_goes_when_the_next_starts")
  else {
    return;
  };
  let project = service_project(implement("Finish."), Vec::new());
  let root = project.repo.path();
  let db = ProjectDb::open(root).expect("open");
  let label = format!("{CONTAINER_PROJECT_LABEL}={}", db.project().id());
  drop(db);
  let run = RunId::generate();
  let left = ContainerName::of(run, &ServiceName::new("db").expect("a name"), 40_001);
  let unnoted = ContainerName::of(run, &ServiceName::new("cache").expect("a name"), 40_002);
  for name in [&left, &unnoted] {
    start_detached(&docker, name, &label);
  }
  let notes = container_notes_folder(root, run);
  fs::create_dir_all(&notes).expect("notes");
  fs::write(notes.join(left.as_str()), "").expect("a note");
  let labelled = || docker.labelled(CONTAINER_PROJECT_LABEL, label.split_once('=').expect("a label").1);

  let (_scheduler, _events) = scheduler(root);
  eventually("the noted container a runner left is removed", || {
    !labelled().expect("listed").contains(&left.as_str().to_owned())
  })
  .await;
  eventually("its note goes", || !notes.join(left.as_str()).exists()).await;
  assert!(
    labelled().expect("listed").contains(&unnoted.as_str().to_owned()),
    "a container no run notes is left alone"
  );
  docker.remove(unnoted.as_str()).expect("cleaned up");
}

/// Starts [`IMAGE`] as `name` with `label`, in the background, as a runner that died would have left it.
fn start_detached(docker: &Docker, name: &ContainerName, label: &str) {
  let command = docker.command(["run", "--detach", "--name", name.as_str(), "--label", label, IMAGE]);
  let output = ProcessRunner::new(Duration::from_secs(60))
    .run(&command)
    .expect("docker runs");
  assert!(output.success(), "the container starts: {}", output.stderr);
}
