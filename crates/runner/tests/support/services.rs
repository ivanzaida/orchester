//! Projects whose steps start services through `start_service`, for the service tests: each service runs
//! `orchester-test-command` as a stand-in server (`fake_server`), which reports its pid, and the pid of a child it
//! starts, in `<repo>/.orchester/service-report/`.

use std::{
  fs,
  net::{Ipv4Addr, TcpStream},
  path::{Path, PathBuf},
};

use orchester_core::{
  id::RunId,
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{CommandArg, CommandProgram, ResourceBody, Slug},
  secret::Secret,
  service::{Readiness, ReadyCheck, ReadyTimeout, Service, ServiceName, Stack, StackName},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use super::{PATIENCE, Setup, StepSpec, TestProject, project, qa::calls, tools::TEST_COMMAND};

/// Where the stand-in servers report their pids.
pub fn report_folder(root: &Path) -> PathBuf {
  root.join(".orchester").join("service-report")
}

/// The file `what` of the report folder, as a service's argument names it.
pub fn report_file(root: &Path, what: &str) -> String {
  report_folder(root).join(what).to_string_lossy().into_owned()
}

/// The service `name` running `orchester-test-command` with `arguments`, ready by `check` within `timeout` seconds.
pub fn service(name: &str, arguments: &[String], check: ReadyCheck, timeout: u64) -> Service {
  let readiness = Readiness::new(check, ReadyTimeout::from_secs(timeout).expect("in range"));
  let program = CommandProgram::new(TEST_COMMAND).expect("the test program");
  let arguments = arguments
    .iter()
    .map(|argument| CommandArg::new(argument).expect("an argument"))
    .collect();
  Service::new(ServiceName::new(name).expect("a name"), program, readiness).with_arguments(arguments)
}

/// A project of `steps` whose agents write the worktree and reach the network, as `start_service` needs, with
/// `secrets` and the report folder; its services are published with [`publish`].
pub fn service_project(steps: Vec<StepSpec>, secrets: Vec<(Secret, &'static str)>) -> TestProject {
  let project = project(&Setup {
    steps,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AnyInWorktree,
      NetworkAccess::On,
    )),
    secrets,
    ..Setup::new("Try the service.")
  });
  fs::create_dir_all(report_folder(project.repo.path())).expect("report folder");
  project
}

/// Publishes `services` in the project at `root`, as a person does on the project's Services page.
pub fn publish(root: &Path, services: &[Service]) {
  for service in services {
    publish_one(root, service.name().as_str(), service);
  }
}

/// Publishes the stack `name` of `services` in the project at `root`, after the services it lists.
pub fn publish_stack(root: &Path, name: &str, services: &[&str]) {
  let services = services
    .iter()
    .map(|service| ServiceName::new(service).expect("a name"))
    .collect();
  let stack = Stack::new(StackName::new(name).expect("a name"), services).expect("a stack");
  publish_one(root, name, &stack);
}

fn publish_one<B: ResourceBody>(root: &Path, name: &str, body: &B) {
  let mut db = ProjectDb::open(root).expect("open");
  let resource = db
    .create_resource(
      Slug::new(name).expect("slug"),
      Name::new(name).expect("name"),
      body,
      Timestamp::now(),
    )
    .expect("resource");
  db.publish_draft::<B>(resource.id(), Timestamp::now())
    .expect("published");
}

/// The answer of the run's first call of `tool`, `<tool> <ok|error>: <first line>`, once it returned.
pub async fn answered(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId, tool: &str) -> String {
  tokio::time::timeout(PATIENCE, async {
    loop {
      if let Some(call) = calls(root, run).into_iter().find(|call| call.starts_with(tool)) {
        return call;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .unwrap_or_else(|_| panic!("{tool} answers in time"))
}

/// The port of a `start_service ok: {"port":…,"url":…}` answer, checked against its URL.
pub fn started_port(answer: &str) -> u16 {
  let json = answer
    .strip_prefix("start_service ok: ")
    .unwrap_or_else(|| panic!("the service started: {answer}"));
  let started: serde_json::Value = serde_json::from_str(json).expect("the answer is JSON");
  let port = u16::try_from(started["port"].as_u64().expect("a port")).expect("a port number");
  assert_eq!(started["url"], format!("http://127.0.0.1:{port}"));
  port
}

/// Whether something accepts connections on loopback port `port`.
pub fn accepts(port: u16) -> bool {
  TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_ok()
}

/// Waits until nothing accepts connections on loopback port `port`.
pub async fn closed(port: u16, what: &str) {
  let waited = tokio::time::timeout(PATIENCE, async {
    while accepts(port) {
      tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
  })
  .await;
  assert!(waited.is_ok(), "{what} (port {port}) still accepts connections");
}

/// The pid the stand-in reported in `what`, once it did.
pub async fn reported_pid(root: &Path, what: &str) -> String {
  let file = report_folder(root).join(what);
  tokio::time::timeout(PATIENCE, async {
    loop {
      if let Ok(pid) = fs::read_to_string(&file)
        && !pid.trim().is_empty()
      {
        return pid.trim().to_owned();
      }
      tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
  })
  .await
  .expect("the stand-in reports its pid")
}
