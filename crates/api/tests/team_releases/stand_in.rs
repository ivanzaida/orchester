//! The stand-in team server takes an Admin's release writes and a task's moves, and answers the release reads, in the
//! contract's shapes.

use std::{
  io::{Read, Write},
  net::TcpStream,
};

use orchester_api::{
  ChangelogDocument, ChangelogResponse, ReleaseDeleted, ReleaseMarked, ReleaseResponse, SharedChangelog, SharingError,
  SharingErrorDetails,
  test_support::team_server::{AuthorityTask, CODE, Standing, TeamServer},
  v2::{ModuleSummary, Page, ReleaseBurndownRead, ReleaseHistoryPage, ReleaseSummary, TaskRead},
};
use orchester_core::{
  docs::blocks::Body,
  id::{IdempotencyKey, ReleaseId},
  module::Horizon,
  release::ReleaseState,
  task::organization::ChangeActor,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// A signed-in Admin of the stand-in's one project, which has the tasks numbered 1 (Done), 2 (In progress) and 3
/// (Canceled).
struct Team {
  server: TeamServer,
  token: String,
  project: String,
}

impl Team {
  fn start(standing: Standing) -> Self {
    let server = TeamServer::start();
    server.share(&[("Kontur", "kontur", None)]);
    server.stand_as(standing);
    let task = |number, workflow| AuthorityTask {
      number,
      title: format!("Task {number}"),
      workflow,
      priority: "medium",
      pipeline: None,
      module: None,
      created_minutes_ago: 60,
      id: None,
      assignee: None,
      claim: None,
      work: None,
      blocked_reason: None,
    };
    server.authority_tasks(vec![task(1, "done"), task(2, "in_progress"), task(3, "canceled")]);
    let project = server.project("kontur").expect("the shared project").to_string();
    let mut team = Self {
      server,
      token: String::new(),
      project,
    };
    let (status, joined) = team.call("POST", "/api/v1/auth/code", &json!({"code": CODE}));
    assert_eq!(status, 201, "{joined}");
    joined["token"].as_str().expect("a token").clone_into(&mut team.token);
    team
  }

  /// `method` on `path` with `body`: the status and the answer.
  fn call(&self, method: &str, path: &str, body: &Value) -> (u16, Value) {
    let address = self.server.url().trim_start_matches("http://").to_owned();
    let mut stream = TcpStream::connect(address).expect("the stand-in listens");
    let body = body.to_string();
    let request = format!(
      "{method} {path} HTTP/1.1\r\nHost: stand-in\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\n\
       Content-Length: {}\r\n\r\n{body}",
      self.token,
      body.len()
    );
    stream.write_all(request.as_bytes()).expect("send");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).expect("an answer");
    let (head, body) = answer.split_once("\r\n\r\n").expect("a head and a body");
    let status = head
      .split_whitespace()
      .nth(1)
      .and_then(|code| code.parse().ok())
      .expect("a status");
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
  }

  fn v1(&self, method: &str, rest: &str, body: &Value) -> (u16, Value) {
    self.call(method, &format!("/api/v1/projects/{}/{rest}", self.project), body)
  }

  fn v2(&self, method: &str, rest: &str, body: &Value) -> (u16, Value) {
    self.call(method, &format!("/api/v2/projects/{}/{rest}", self.project), body)
  }

  /// A planned release named `name`, planned on `planned`.
  fn create(&self, name: &str, planned: Option<&str>) -> ReleaseId {
    let id = ReleaseId::generate();
    let body = json!({"idempotency_key": IdempotencyKey::generate(), "id": id,
      "release": {"name": name, "description": "", "planned": planned}});
    let (status, answer) = self.v1("POST", "releases", &body);
    assert_eq!(status, 201, "{answer}");
    id
  }

  /// The task numbered `number` as the task list reads it.
  fn task(&self, number: u64) -> Value {
    let (status, page) = self.v2("POST", "task-list", &json!({}));
    assert_eq!(status, 200, "{page}");
    let tasks = page["items"].as_array().expect("tasks");
    tasks
      .iter()
      .find(|task| task["number"] == number)
      .expect("the task")
      .clone()
  }

  /// Moves the task numbered `number` to `release`: the status and the answer.
  fn move_task(&self, number: u64, release: Option<ReleaseId>) -> (u16, Value) {
    let task = self.task(number);
    let id = task["id"].as_str().expect("an id");
    let body = json!({"revision": task["revision"], "changes": {"release": release}});
    self.v1("PATCH", &format!("tasks/{id}"), &body)
  }

  fn summary(&self, release: ReleaseId) -> ReleaseSummary {
    read(self.v2("GET", &format!("releases/{release}"), &Value::Null))
  }
}

/// `answer` read as `T`, after a 200.
fn read<T: DeserializeOwned>((status, answer): (u16, Value)) -> T {
  assert_eq!(status, 200, "{answer}");
  serde_json::from_value(answer).expect("the contract's shape")
}

#[test]
fn stand_in_release_definitions_are_admin_only() {
  let team = Team::start(Standing::Member);
  let body = json!({"idempotency_key": IdempotencyKey::generate(), "id": ReleaseId::generate(),
    "release": {"name": "0.4.0"}});
  let (status, refused) = team.v1("POST", "releases", &body);
  assert_eq!((status, refused["error"].as_str()), (403, Some("not_admin")));
}

#[test]
fn stand_in_release_create_and_change_with_their_refusals() {
  let team = Team::start(Standing::Admin);
  let (id, key) = (ReleaseId::generate(), IdempotencyKey::generate());
  let create = json!({"idempotency_key": key, "id": id, "release": {"name": "0.4.0", "planned": "2026-11-15"}});
  let (status, first) = team.v1("POST", "releases", &create);
  assert_eq!(status, 201, "{first}");
  let created: ReleaseResponse = serde_json::from_value(first.clone()).expect("a release");
  assert_eq!(
    (created.release.id, created.release.state()),
    (id, ReleaseState::Planned)
  );
  assert_eq!(
    team.v1("POST", "releases", &create),
    (201, first),
    "a repeat gets the first answer"
  );

  let named = json!({"idempotency_key": IdempotencyKey::generate(), "id": ReleaseId::generate(),
    "release": {"name": "0.4.0 ".to_uppercase()}});
  let (status, exists) = team.v1("POST", "releases", &named);
  assert_eq!(status, 409);
  let exists: SharingError = serde_json::from_value(exists).expect("release_exists");
  assert!(matches!(exists.details, SharingErrorDetails::Release { release } if release.id == id));
  let taken = json!({"idempotency_key": IdempotencyKey::generate(), "id": id, "release": {"name": "0.5.0"}});
  assert_eq!(team.v1("POST", "releases", &taken).1["error"], "id_taken");
  let blank = json!({"idempotency_key": IdempotencyKey::generate(), "id": ReleaseId::generate(),
    "release": {"name": " "}});
  assert_eq!(
    team.v1("POST", "releases", &blank),
    (422, json!({"error": "invalid", "message": "invalid"}))
  );
  let unknown = json!({"idempotency_key": IdempotencyKey::generate(), "id": ReleaseId::generate(),
    "release": {"name": "0.6.0", "state": "released"}});
  assert_eq!(team.v1("POST", "releases", &unknown).0, 400);

  let change = |revision: u64| {
    json!({"idempotency_key": IdempotencyKey::generate(), "revision": revision,
      "release": {"name": "0.4.0", "description": "The *Docs* release.", "planned": "2026-11-20"}})
  };
  let changed: ReleaseResponse = read(team.v1("PATCH", &format!("releases/{id}"), &change(1)));
  assert_eq!(changed.release.revision.get(), 2);
  assert_eq!(changed.release.fields.planned.as_deref(), Some("2026-11-20"));
  let (status, stale) = team.v1("PATCH", &format!("releases/{id}"), &change(1));
  assert_eq!(status, 409);
  let stale: SharingError = serde_json::from_value(stale).expect("revision_conflict");
  assert!(matches!(stale.details, SharingErrorDetails::ReleaseRevision { current } if current.revision.get() == 2));
}

#[test]
fn stand_in_release_moves_tasks_and_marks_released() {
  let team = Team::start(Standing::Admin);
  let (release, next) = (
    team.create("0.4.0", Some("2026-11-15")),
    team.create("0.5.0", Some("2026-12-01")),
  );
  team.create("Beta", None);
  for number in [1, 2, 3] {
    assert_eq!(team.move_task(number, Some(release)).0, 200);
  }
  let (status, refused) = team.move_task(1, Some(ReleaseId::generate()));
  assert_eq!((status, &refused["reason"]), (422, &json!("release_not_found")));
  let summary = team.summary(release);
  assert_eq!(
    (summary.tasks, summary.progress.scope, summary.progress.done),
    (3, 2, 1)
  );
  let filters = json!({"filters": {"releases": [release]}});
  let page: Page<TaskRead> = read(team.v2("POST", "task-list", &filters));
  assert_eq!(page.items.len(), 3);

  let mark = json!({"idempotency_key": IdempotencyKey::generate(), "revision": 1,
    "commit": "3f2a9c1e0b4d5a6f7e8d9c0b1a2f3e4d5c6b7a89"});
  let marked: ReleaseMarked = read(team.v1("POST", &format!("releases/{release}/released"), &mark));
  assert_eq!(marked.release.state(), ReleaseState::Released);
  let mark_made = marked.release.released.as_ref().expect("released");
  assert!(mark_made.commit.is_some());
  assert_eq!(marked.moves.len(), 1, "only the unfinished task moves");
  assert_eq!(
    (marked.moves[0].from, marked.moves[0].to, marked.moves[0].actor),
    (Some(release), Some(next), ChangeActor::Orchester)
  );
  assert_eq!(team.summary(next).tasks, 1);

  let (status, refused) = team.move_task(1, None);
  assert_eq!((status, &refused["reason"]), (422, &json!("released")));
  let delete = json!({"idempotency_key": IdempotencyKey::generate(), "revision": marked.release.revision});
  assert_eq!(
    team.v1("DELETE", &format!("releases/{release}"), &delete).1["reason"],
    "released"
  );

  let history: ReleaseHistoryPage = read(team.v2("POST", &format!("releases/{release}/history"), &json!({})));
  assert_eq!(history.items.len(), 4, "three moves in, one out");
  let released: Page<ReleaseSummary> = read(team.v2("POST", "release-list", &json!({"state": "released"})));
  assert_eq!(released.items.len(), 1);
  let all: Page<ReleaseSummary> = read(team.v2("POST", "release-list", &json!({})));
  assert_eq!(all.items.len(), 3);
  let modules: Page<ModuleSummary> = read(team.v2("POST", "module-list", &json!({})));
  assert_eq!(modules.items[0].module.module.fields.horizon, Horizon::Later);
}

#[test]
fn stand_in_release_changelog_burndown_and_delete() {
  let team = Team::start(Standing::Admin);
  let release = team.create("0.4.0", Some("2026-11-15"));
  assert_eq!(team.move_task(2, Some(release)).0, 200);
  let document = ChangelogDocument::of_body(&Body::parse("## Other\n\n- Task 2\n")).expect("a document");
  let patch = || {
    json!({"idempotency_key": IdempotencyKey::generate(), "revision": 1, "document": document,
      "hidden": [], "kept_aside": []})
  };
  let changed: ChangelogResponse = read(team.v1("PATCH", &format!("releases/{release}/changelog"), &patch()));
  assert_eq!(
    (changed.changelog.revision.get(), &changed.changelog.document),
    (2, &document)
  );
  let changelog: SharedChangelog = read(team.v2("GET", &format!("releases/{release}/changelog"), &Value::Null));
  assert_eq!(changelog, changed.changelog);
  let (status, stale) = team.v1("PATCH", &format!("releases/{release}/changelog"), &patch());
  assert_eq!((status, &stale["error"]), (409, &json!("revision_conflict")));

  let burndown: ReleaseBurndownRead = read(team.v2("GET", &format!("releases/{release}/burndown"), &Value::Null));
  assert_eq!(burndown.release, release);
  assert_eq!(burndown.days.iter().rev().find_map(|day| day.remaining), Some(1));

  let delete = json!({"idempotency_key": IdempotencyKey::generate(), "revision": 2});
  let deleted: ReleaseDeleted = read(team.v1("DELETE", &format!("releases/{release}"), &delete));
  assert_eq!((deleted.moves.len(), deleted.moves[0].to), (1, None));
  assert_eq!(team.v2("GET", &format!("releases/{release}"), &Value::Null).0, 404);
  assert_eq!(team.task(2)["release"], Value::Null);
}
