//! Needs and stacks end to end against `orchester-fake-codex`, with `orchester-test-command` as the services
//! (`support::services`): `start_service` starts what a service needs first, in order, each once what it needs is
//! ready, and fills `{{services.<name>.url|port}}` with the URLs and ports of the services started before it;
//! `start_stack` starts a stack's services and their needs, each once. A service the step runs already is reused, not
//! started twice. When a service a start needs fails, nothing after it starts and the error names it, its exit code
//! and its last lines; a readiness timeout is noted in the conversation too. When a ready service crashes, the
//! services that need it stop, the agent's next tool answer carries a notice and the conversation records it. Needs
//! `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test service_needs service`.

mod support;

use std::{fs, path::Path};

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::{IdempotencyKey, RunId},
  run::{Answerer, ApprovalAnswer, RunStatus},
  service::{
    LogPattern, ReadyAsked, ReadyCheck, Service, ServiceCrash, ServiceName, ServiceStartFailure, StartFailureCause,
  },
  store::ActivityStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{
  PATIENCE, StepSpec, is_running, pending_approval,
  qa::{approve, calls, gone},
  reported, scheduler,
  services::{
    accepts, answered, publish, publish_stack, report_file, report_folder, reported_pid, service, service_project,
    started_port,
  },
  wait_for,
};

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

/// The service `name` that needs `needs`: it reports its pid in `<name>.pid`, checks that each need accepts
/// connections, writes each need's URL to `<name>-<need>.url`, then listens on its port, ready once it accepts
/// connections.
fn chained(root: &Path, name: &str, needs: &[&str]) -> Service {
  let mut arguments = vec!["pid-file".to_owned(), report_file(root, &format!("{name}.pid"))];
  for need in needs {
    arguments.extend([
      "connect".to_owned(),
      format!("{{{{services.{need}.port}}}}"),
      "write".to_owned(),
      report_file(root, &format!("{name}-{need}.url")),
      format!("{{{{services.{need}.url}}}}"),
    ]);
  }
  arguments.extend(["listen".to_owned(), "{{port}}".to_owned()]);
  let needs = needs
    .iter()
    .map(|need| ServiceName::new(need).expect("a name"))
    .collect();
  service(name, &arguments, ReadyCheck::PortOpen, 20)
    .with_needs(needs)
    .expect("needs")
}

/// Publishes the chain `db` ← `api` ← `web`: `web` needs `api`, which needs `db`.
fn publish_chain(root: &Path) {
  publish(
    root,
    &[
      chained(root, "db", &[]),
      chained(root, "api", &["db"]),
      chained(root, "web", &["api"]),
    ],
  );
}

/// The port of the URL `name` wrote for its need `need`, `http://127.0.0.1:<port>`.
fn wired_port(root: &Path, name: &str, need: &str) -> u16 {
  let url = fs::read_to_string(report_folder(root).join(format!("{name}-{need}.url"))).expect("the wired URL");
  url
    .strip_prefix("http://127.0.0.1:")
    .and_then(|port| port.parse().ok())
    .unwrap_or_else(|| panic!("`{url}` is a loopback URL"))
}

/// The answers of the run's first `count` calls of `start_service`, once they returned.
async fn starts(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId, count: usize) -> Vec<String> {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let starts: Vec<String> = calls(root, run)
        .into_iter()
        .filter(|call| call.starts_with("start_service"))
        .collect();
      if starts.len() >= count {
        return starts;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .unwrap_or_else(|_| panic!("{count} starts answer in time"))
}

/// Answers the run's pending approval with Approve similar, so its later calls of the tool do not ask.
async fn approve_similar(scheduler: &Scheduler, root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) {
  let request = pending_approval(root, events, run).await;
  scheduler
    .decide_tool_approval(run, request.id, ApprovalAnswer::Similar, Answerer::Person)
    .await
    .expect("approved");
}

#[tokio::test(flavor = "multi_thread")]
async fn service_needs_start_in_order_and_wire_their_urls() {
  let project = service_project(implement("Start the service web. Hold the services."), Vec::new());
  let root = project.repo.path();
  publish_chain(root);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let web = started_port(&answered(root, &mut events, id, "start_service").await);
  // Each service checked that what it needs accepts connections before it went on, so each started once its needs
  // were ready; and each got the URL of what it needs.
  let (api, db) = (wired_port(root, "web", "api"), wired_port(root, "api", "db"));
  for port in [db, api, web] {
    assert!(accepts(port), "port {port} is listened on");
  }
  assert!(db != api && api != web && db != web, "{db} {api} {web}");
  let mut pids = Vec::new();
  for name in ["db", "api", "web"] {
    let pid = reported_pid(root, &format!("{name}.pid")).await;
    assert!(is_running(&pid), "{name} runs while its step works");
    pids.push(pid);
  }

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  for pid in &pids {
    gone(pid, "a service after its step ended").await;
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn service_already_running_is_reused_not_started_twice() {
  let project = service_project(
    implement("Start the service api. Then start the service web. Hold the services."),
    Vec::new(),
  );
  let root = project.repo.path();
  publish_chain(root);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve_similar(&scheduler, root, &mut events, id).await;
  let api = started_port(&answered(root, &mut events, id, "start_service").await);
  let (db_pid, api_pid) = (reported_pid(root, "db.pid").await, reported_pid(root, "api.pid").await);
  let starts = starts(root, &mut events, id, 2).await;
  let web = started_port(&starts[1]);
  let web_pid = reported_pid(root, "web.pid").await;
  assert!(accepts(web), "web listens on the port it was given");
  // `web` got the `api` the first start started, which still runs with the same `db`.
  assert_eq!(wired_port(root, "web", "api"), api);
  assert_eq!(
    reported_pid(root, "api.pid").await,
    api_pid,
    "api was not started again"
  );
  assert_eq!(reported_pid(root, "db.pid").await, db_pid, "db was not started again");
  for pid in [&db_pid, &api_pid, &web_pid] {
    assert!(is_running(pid), "every service runs");
  }

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn service_stack_starts_its_services_and_their_needs_once() {
  let project = service_project(implement("Start the stack full. Hold the services."), Vec::new());
  let root = project.repo.path();
  publish_chain(root);
  // `db` is in the stack and a need of `api`: it starts once.
  publish_stack(root, "full", &["web", "db"]);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = answered(root, &mut events, id, "start_stack").await;
  let json = answer
    .strip_prefix("start_stack ok: ")
    .unwrap_or_else(|| panic!("the stack started: {answer}"));
  let started: serde_json::Value = serde_json::from_str(json).expect("the answer is JSON");
  let services = started["services"].as_object().expect("the services");
  let mut names: Vec<&str> = services.keys().map(String::as_str).collect();
  names.sort_unstable();
  assert_eq!(names, ["api", "db", "web"]);
  let port = |name: &str| u16::try_from(services[name]["port"].as_u64().expect("a port")).expect("a port number");
  assert_eq!(wired_port(root, "api", "db"), port("db"));
  assert_eq!(wired_port(root, "web", "api"), port("api"));
  for name in ["db", "api", "web"] {
    assert!(accepts(port(name)), "{name} listens on the port it was given");
    assert_eq!(services[name]["url"], format!("http://127.0.0.1:{}", port(name)));
  }

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn service_failing_need_stops_the_chain_and_is_named() {
  let project = service_project(implement("Start the service web."), Vec::new());
  let root = project.repo.path();
  let failing = service(
    "db",
    &[
      "pid-file".to_owned(),
      report_file(root, "db.pid"),
      "stderr".to_owned(),
      "no disk".to_owned(),
      "exit".to_owned(),
      "3".to_owned(),
    ],
    ReadyCheck::PortOpen,
    20,
  );
  publish(
    root,
    &[failing, chained(root, "api", &["db"]), chained(root, "web", &["api"])],
  );
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = answered(root, &mut events, id, "start_service").await;
  assert!(
    answer.starts_with(
      "start_service error: the service `db` did not start, so nothing after it was started (`api`, `web`): the \
       service `db` exited with code 3 before it was ready"
    ),
    "{answer}"
  );
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert!(report_folder(root).join("db.pid").exists(), "db was started");
  for name in ["api", "web"] {
    assert!(
      !report_folder(root).join(format!("{name}.pid")).exists(),
      "{name} was not started after db failed"
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn service_failing_middle_need_is_the_root_cause_with_its_last_lines() {
  let project = service_project(implement("Start the service web."), Vec::new());
  let root = project.repo.path();
  let failing = service(
    "api",
    &[
      "pid-file".to_owned(),
      report_file(root, "api.pid"),
      "stderr".to_owned(),
      "the schema is missing".to_owned(),
      "exit".to_owned(),
      "4".to_owned(),
    ],
    ReadyCheck::PortOpen,
    20,
  )
  .with_needs(vec![ServiceName::new("db").expect("a name")])
  .expect("needs");
  publish(
    root,
    &[chained(root, "db", &[]), failing, chained(root, "web", &["api"])],
  );
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = answered(root, &mut events, id, "start_service").await;
  // `db` became ready; `api`, which needs it, is the root cause, with its exit code; `web` is not started.
  assert!(
    answer.starts_with(
      "start_service error: the service `api` did not start, so nothing after it was started (`web`): the service \
       `api` exited with code 4 before it was ready"
    ),
    "{answer}"
  );
  let lines = reported(&ProjectDb::open(root).expect("open"), id, "orchester-answer| ");
  assert!(
    lines.iter().any(|line| line.ends_with("err| the schema is missing")),
    "the error shows the root cause's last lines: {lines:?}"
  );
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert!(report_folder(root).join("db.pid").exists(), "db was started");
  assert!(
    !report_folder(root).join("web.pid").exists(),
    "web was not started after api failed"
  );
}

/// The service `name` that needs `needs`, reports its pid in `<name>.pid` and runs `then`, ready as `check` says.
fn needing(root: &Path, name: &str, needs: &[&str], (then, check): (&[&str], ReadyCheck)) -> Service {
  let mut arguments = vec!["pid-file".to_owned(), report_file(root, &format!("{name}.pid"))];
  arguments.extend(then.iter().map(|argument| (*argument).to_owned()));
  let needs = needs
    .iter()
    .map(|need| ServiceName::new(need).expect("a name"))
    .collect();
  service(name, &arguments, check, 20).with_needs(needs).expect("needs")
}

#[tokio::test(flavor = "multi_thread")]
async fn service_crash_after_ready_notifies_the_agent_and_the_conversation() {
  let project = service_project(
    implement("Start the service web. Then wait for a crash notice."),
    Vec::new(),
  );
  let root = project.repo.path();
  let ready = ReadyCheck::LogLine(LogPattern::new("^ready$").expect("a pattern"));
  let crashing = ["say", "ready", "sleep", "4", "stderr", "lost the disk", "exit", "7"];
  let listen = ["listen", "{{port}}"];
  // `web` needs `api`, which needs `db`: when `db` crashes, both are stopped because of it.
  publish(
    root,
    &[
      needing(root, "db", &[], (&crashing, ready)),
      needing(root, "api", &["db"], (&listen, ReadyCheck::PortOpen)),
      needing(root, "web", &["api"], (&listen, ReadyCheck::PortOpen)),
    ],
  );
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  started_port(&answered(root, &mut events, id, "start_service").await);
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let db = ProjectDb::open(root).expect("open");
  let notice = reported(&db, id, "orchester-notice| ");
  assert!(
    notice.first().is_some_and(|line| line
      .starts_with("Orchester notice: the service `db` crashed: it exited with code 7, ")
      && line
        .ends_with("s after it started. Orchester stopped the services that need it: `api`, `web`. Its last lines:")),
    "{notice:?}"
  );
  assert!(
    notice.iter().any(|line| line.ends_with("err| lost the disk")),
    "{notice:?}"
  );

  let crashes: Vec<ServiceCrash> = db
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem {
        problem: RunnerProblem::ServiceCrashed(crash),
      } => Some(crash),
      _ => None,
    })
    .collect();
  let [crash] = crashes.as_slice() else {
    panic!("one crash is in the conversation: {crashes:?}");
  };
  assert_eq!(crash.service, "db");
  assert_eq!(crash.exit_code, Some(7));
  assert_eq!(crash.stopped, ["api", "web"]);
  assert!(crash.uptime_ms >= 4000, "{crash:?}");
  assert!(
    crash.lines.iter().any(|line| line.ends_with("err| lost the disk")),
    "{crash:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn service_readiness_timeout_is_noted_in_the_conversation_with_its_last_lines() {
  let project = service_project(implement("Start the service web."), Vec::new());
  let root = project.repo.path();
  // `api` never listens on its port, and may take 1 s to.
  let waits: Vec<String> = ["say", "starting", "sleep", "30"].map(str::to_owned).to_vec();
  let slow = service("api", &waits, ReadyCheck::PortOpen, 1)
    .with_needs(vec![ServiceName::new("db").expect("a name")])
    .expect("needs");
  publish(
    root,
    &[
      needing(root, "db", &[], (&["listen", "{{port}}"], ReadyCheck::PortOpen)),
      slow,
      chained(root, "web", &["api"]),
    ],
  );
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let answer = answered(root, &mut events, id, "start_service").await;
  assert!(answer.contains("was not ready within 1 s"), "{answer}");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let failures: Vec<ServiceStartFailure> = ProjectDb::open(root)
    .expect("open")
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem {
        problem: RunnerProblem::ServiceStartFailed(failure),
      } => Some(failure),
      _ => None,
    })
    .collect();
  let [failure] = failures.as_slice() else {
    panic!("one failed start is in the conversation: {failures:?}");
  };
  assert_eq!(failure.service, "api");
  assert!(
    matches!(failure.cause, StartFailureCause::NotReady { secs: 1, asked: ReadyAsked::Port { .. } }),
    "{failure:?}"
  );
  assert_eq!(failure.not_started, ["web"]);
  assert_eq!(failure.running, ["db"]);
  assert!(
    failure.lines.iter().any(|line| line.ends_with("out| starting")),
    "{failure:?}"
  );
}
