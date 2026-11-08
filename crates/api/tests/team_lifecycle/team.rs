//! The stand-in team server of the lifecycle tests: one project, a signed-in caller of the standing a test names, and
//! the calls, reads and work commands the tests make.

use std::{
  io::{Read, Write},
  net::TcpStream,
  time::Duration,
};

use orchester_api::{
  test_support::team_server::{AuthorityClaim, AuthorityModule, AuthorityTask, CODE, Person, Standing, TeamServer},
  v2::{Action, DenialReason, LifecycleAction, TaskRead},
};
use orchester_core::id::{OperationId, RunId, WorkEventId};
use serde_json::{Value, json};

pub const YOU: AuthorityClaim = AuthorityClaim {
  holder: Person::You,
  renewed_minutes_ago: 1,
  stale: false,
  generation: 0,
};
pub const OLHA: AuthorityClaim = AuthorityClaim {
  holder: Person::Named("olha"),
  renewed_minutes_ago: 1,
  stale: false,
  generation: 0,
};

pub fn task(number: u32, workflow: &'static str, claim: Option<AuthorityClaim>) -> AuthorityTask {
  AuthorityTask {
    number,
    title: format!("Task {number}"),
    workflow,
    priority: "medium",
    pipeline: None,
    module: None,
    created_minutes_ago: 60,
    id: None,
    assignee: None,
    claim,
    work: None,
  }
}

pub fn module(claim: Option<AuthorityClaim>) -> AuthorityModule {
  AuthorityModule {
    name: "Payments",
    description: "",
    start: None,
    target: None,
    branch: "module/payments",
    assignee: None,
    claim,
    likely: None,
    pending: None,
  }
}

/// A signed-in caller of the stand-in's one project.
pub struct Team {
  pub server: TeamServer,
  token: String,
  project: String,
}

impl Team {
  pub fn start(standing: Standing, tasks: Vec<AuthorityTask>) -> Self {
    let server = TeamServer::start();
    server.share(&[("Kontur", "kontur", None)]);
    server.stand_as(standing);
    server.authority_tasks(tasks);
    let project = server.project("kontur").expect("the shared project").to_string();
    let mut team = Self {
      server,
      token: String::new(),
      project,
    };
    let (status, joined) = team.call("POST", "/api/v1/auth/code", &json!({"code": CODE}), &[]);
    assert_eq!(status, 201, "{joined}");
    joined["token"].as_str().expect("a token").clone_into(&mut team.token);
    team
  }

  fn call(&self, method: &str, path: &str, body: &Value, headers: &[&str]) -> (u16, Value) {
    let (head, body) = self.raw(method, path, body, headers, None);
    let status = head
      .split_whitespace()
      .nth(1)
      .and_then(|code| code.parse().ok())
      .expect("a status");
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
  }

  fn raw(&self, method: &str, path: &str, body: &Value, headers: &[&str], quiet: Option<Duration>) -> (String, String) {
    let address = self.server.url().trim_start_matches("http://").to_owned();
    let mut stream = TcpStream::connect(address).expect("the stand-in listens");
    stream.set_read_timeout(quiet).expect("a timeout");
    let body = body.to_string();
    let headers: String = headers.iter().map(|header| format!("{header}\r\n")).collect();
    let request = format!(
      "{method} {path} HTTP/1.1\r\nHost: stand-in\r\nAuthorization: Bearer {}\r\n{headers}Content-Type: application/json\r\n\
       Content-Length: {}\r\n\r\n{body}",
      self.token,
      body.len()
    );
    stream.write_all(request.as_bytes()).expect("send");
    let mut answer = Vec::new();
    // An open subscription ends by going quiet; any other answer by closing.
    let _ = stream.read_to_end(&mut answer);
    let answer = String::from_utf8(answer).expect("text");
    let (head, body) = answer.split_once("\r\n\r\n").expect("a head and a body");
    (head.to_owned(), body.to_owned())
  }

  pub fn v2(&self, method: &str, rest: &str, body: &Value) -> (u16, Value) {
    self.call(method, &format!("/api/v2/projects/{}/{rest}", self.project), body, &[])
  }

  /// The task numbered `number` as the task list reads it.
  pub fn task(&self, number: u64) -> Value {
    let (status, page) = self.v2("POST", "task-list", &json!({}));
    assert_eq!(status, 200, "{page}");
    let found = page["items"].as_array().expect("tasks").iter();
    found
      .clone()
      .find(|task| task["number"] == number)
      .expect("the task")
      .clone()
  }

  /// `changes` on the task numbered `number`, on the revision it has now.
  pub fn patch(&self, number: u64, changes: &Value) -> (u16, Value) {
    let task = self.task(number);
    self.patch_on(&task, task["revision"].as_u64().expect("a revision"), changes)
  }

  pub fn patch_on(&self, task: &Value, revision: u64, changes: &Value) -> (u16, Value) {
    let path = format!(
      "/api/v1/projects/{}/tasks/{}",
      self.project,
      task["id"].as_str().expect("an id")
    );
    self.call("PATCH", &path, &json!({"revision": revision, "changes": changes}), &[])
  }

  /// The commands of `POST …/commands`, each under an operation of its own.
  pub fn command(&self, command: &Value) -> (u16, Value) {
    let operation = OperationId::generate();
    let body = json!({"operation_id": operation, "expected": [], "command": command});
    self.v2("POST", "commands", &body)
  }

  /// Every batch the stand-in committed, as the subscription replays them: the scopes of each.
  pub fn batches(&self) -> Vec<Vec<Value>> {
    let path = format!("/api/v2/projects/{}/subscription", self.project);
    let headers = ["Last-Event-ID: 0"];
    let (_, events) = self.raw("GET", &path, &Value::Null, &headers, Some(Duration::from_millis(400)));
    events
      .lines()
      .filter_map(|line| line.strip_prefix("data: "))
      .filter_map(|data| serde_json::from_str::<Value>(data).ok())
      .filter(|event| event["kind"] == "invalidated")
      .map(|event| event["scopes"].as_array().cloned().unwrap_or_default())
      .collect()
  }
}

pub fn workflow(task: &Value) -> &str {
  task["workflow"].as_str().expect("a workflow")
}

/// What the task's capabilities say of its lifecycle actions: whether each is allowed, or why not.
pub fn lifecycle(task: &Value) -> Vec<(LifecycleAction, Option<DenialReason>)> {
  let read: TaskRead = serde_json::from_value(task.clone()).expect("a task read");
  LifecycleAction::ALL
    .into_iter()
    .map(|action| {
      let capability = read
        .capabilities
        .actions
        .iter()
        .find(|capability| *capability.action() == Action::Task(action))
        .unwrap_or_else(|| panic!("{action:?} is listed"));
      (action, capability.reason())
    })
    .collect()
}

pub fn claim_id(generation: u8, number: u32) -> String {
  format!("0191c7c0-0000-7000-8000-00003{generation:02x}{number:05x}")
}

pub fn task_id(number: u32) -> String {
  format!("0191c7c0-0000-7000-8000-0000001{number:05x}")
}

pub fn authority(number: u32) -> Value {
  json!({"subject": {"kind": "task", "id": task_id(number)}, "claim_id": claim_id(0, number),
    "work_id": null})
}

/// Starts the work of the task numbered `number` that `YOU` hold: the authority naming its work.
pub fn start(team: &Team, number: u32) -> Value {
  let start = json!({"kind": "work.start", "authority": authority(number), "base_commit": "b".repeat(40)});
  let (status, started) = team.command(&start);
  assert_eq!(status, 201, "{started}");
  let mut named = authority(number);
  named["work_id"] = started["result"]["work"]["id"].clone();
  named
}

/// Reports `outcome` of the run of the work `named`, as its report number `sequence`.
pub fn report(team: &Team, named: &Value, sequence: u64, (outcome, artifacts): (&Value, &Value)) {
  let report = json!({"kind": "work.report", "authority": named, "run_id": RunId::generate(),
    "event_id": WorkEventId::generate(), "sequence": sequence, "outcome": outcome, "artifacts": artifacts});
  let (status, reported) = team.command(&report);
  assert_eq!(status, 201, "{reported}");
}

/// Starts the work of the task numbered `number` that `YOU` hold, and reports its run `outcome`.
pub fn run(team: &Team, number: u32, outcome: &Value, artifacts: &Value) {
  let named = start(team, number);
  report(team, &named, 1, (outcome, artifacts));
}
pub fn report_artifact() -> Value {
  json!([{"sha256": "a".repeat(64), "kind": "report", "size_bytes": 8}])
}

pub fn patch_artifact() -> Value {
  json!([{"sha256": "b".repeat(64), "kind": "patch", "size_bytes": 8}, {"sha256": "a".repeat(64), "kind": "report", "size_bytes": 8}])
}
