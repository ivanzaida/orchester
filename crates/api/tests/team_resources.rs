//! The stand-in team server keeps an MCP server resource: it takes the kind through create, publish, rename and
//! archive, serves its versions, refuses a body with a path, a value or a local id, and computes `widens`; and it
//! answers used-by from its own resources and the runs a test pins.

use std::{
  io::{Read, Write},
  net::TcpStream,
};

use orchester_api::{
  ResourceCreated, Resources, SharedResource, SharedResourceKind, SharedVersion, UsedBy, Versions,
  test_support::team_server::{CODE, Person, PinnedRun, Standing, TeamServer},
};
use orchester_core::{
  id::{IdempotencyKey, ResourceId, RunId},
  resource::VersionNumber,
};
use serde_json::{Value, json};

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

  fn resources(&self, method: &str, rest: &str, body: &Value) -> (u16, Value) {
    self.call(
      method,
      &format!("/api/v1/projects/{}/resources{rest}", self.project),
      body,
    )
  }
}

fn server_body(program: &str) -> Value {
  json!({"name":"knowledge","launch":{"kind":"program","program":program,"args":["-y"],"variables":["TOKEN"]}})
}

fn create(id: ResourceId, body: &Value) -> Value {
  json!({"idempotency_key":IdempotencyKey::generate(),"id":id,"kind":"mcp_server","slug":"knowledge",
    "name":"Knowledge","version":{"body":body,"references":[]}})
}

fn publish(based_on: u32, body: &Value) -> Value {
  json!({"idempotency_key":IdempotencyKey::generate(),"based_on":based_on,"body":body,"references":[]})
}

#[test]
fn stand_in_stores_and_serves_a_team_mcp_server() {
  let team = Team::start(Standing::Admin);
  let id = ResourceId::generate();
  let (status, created) = team.resources("POST", "", &create(id, &server_body("npx")));
  assert_eq!(status, 201, "{created}");
  let created: ResourceCreated = serde_json::from_value(created).expect("created");
  assert_eq!(created.resource.kind, SharedResourceKind::McpServer);
  assert!(!created.version.widens, "the first version does not widen");

  let (status, widened) = team.resources("POST", &format!("/{id}/versions"), &publish(1, &server_body("uvx")));
  assert_eq!(status, 201, "{widened}");
  let widened: SharedVersion = serde_json::from_value(widened["version"].clone()).expect("version");
  assert!(widened.widens, "another program widens");
  assert_eq!(widened.number.get(), 2);

  let (status, stale) = team.resources("POST", &format!("/{id}/versions"), &publish(1, &server_body("npx")));
  assert_eq!((status, stale["error"].as_str()), (409, Some("version_conflict")));

  let (_, listed) = team.resources("GET", "", &Value::Null);
  let listed: Resources = serde_json::from_value(listed).expect("list");
  assert_eq!(listed.resources.len(), 1);
  assert_eq!(listed.resources[0].latest.map(|latest| latest.get()), Some(2));

  let (status, versions) = team.resources("GET", &format!("/{id}/versions?after=1"), &Value::Null);
  assert_eq!(status, 200, "{versions}");
  let versions: Versions = serde_json::from_value(versions).expect("versions");
  assert_eq!(versions.versions.len(), 1);
  assert_eq!(versions.versions[0].body, server_body("uvx"));

  let patch =
    json!({"idempotency_key":IdempotencyKey::generate(),"revision":1,"name":"Knowledge base","archived":true});
  let (status, patched) = team.resources("PATCH", &format!("/{id}"), &patch);
  assert_eq!(status, 200, "{patched}");
  let patched: SharedResource = serde_json::from_value(patched["resource"].clone()).expect("resource");
  assert!(patched.archived);
  assert_eq!(patched.name, "Knowledge base");
  assert_eq!(team.server.resources("kontur").len(), 1);
}

#[test]
fn stand_in_refuses_a_team_mcp_server_with_a_path_a_value_or_an_id() {
  let team = Team::start(Standing::Admin);
  let mut with_value = server_body("npx");
  with_value["launch"]["variables"] = json!({"TOKEN":"s3cret"});
  let mut with_id = server_body("npx");
  with_id["connection"] = json!("0191c7c0-0000-7000-8000-000000000001");
  for body in [server_body(r"C:\tools\npx.exe"), with_value, with_id] {
    let (status, answer) = team.resources("POST", "", &create(ResourceId::generate(), &body));
    assert_eq!((status, answer["reason"].as_str()), (422, Some("body")), "{body}");
  }
  assert!(team.server.resources("kontur").is_empty());
}

#[test]
fn stand_in_leaves_a_member_without_writes_and_reuses_an_answer_for_a_repeated_key() {
  let member = Team::start(Standing::Member);
  let (status, refused) = member.resources("POST", "", &create(ResourceId::generate(), &server_body("npx")));
  assert_eq!((status, refused["error"].as_str()), (403, Some("not_admin")));

  let admin = Team::start(Standing::Admin);
  let body = create(ResourceId::generate(), &server_body("npx"));
  let (first, answer) = admin.resources("POST", "", &body);
  let (second, again) = admin.resources("POST", "", &body);
  assert_eq!((first, second), (201, 201));
  assert_eq!(answer, again);
  assert_eq!(admin.server.resources("kontur").len(), 1);
}

fn named(id: ResourceId, kind: &str, slug: &str, name: &str, body: &Value) -> Value {
  json!({"idempotency_key":IdempotencyKey::generate(),"id":id,"kind":kind,"slug":slug,"name":name,
    "version":{"body":body,"references":[]}})
}

fn pin(task_key: &str, number: u32, ended: bool, pins: &[ResourceId]) -> PinnedRun {
  PinnedRun {
    task_key: task_key.to_owned(),
    run: RunId::generate(),
    number,
    holder: Person::Named("Dana Ruiz"),
    ended,
    pins: pins.iter().map(|id| (*id, VersionNumber::FIRST)).collect(),
  }
}

#[test]
fn stand_in_answers_used_by_with_the_latest_versions_that_name_it_and_the_runs_that_have_not_ended() {
  let team = Team::start(Standing::Admin);
  let (tool, other) = (ResourceId::generate(), ResourceId::generate());
  let (pipeline, profile, archived, unrelated) = (
    ResourceId::generate(),
    ResourceId::generate(),
    ResourceId::generate(),
    ResourceId::generate(),
  );
  let names = |id: ResourceId| json!({"steps":[{"tool":id}]});
  for (id, kind, slug, name, body) in [
    (tool, "tool", "deploy", "Deploy", json!({})),
    (other, "tool", "lint", "Lint", json!({})),
    (profile, "agent_profile", "coder", "Coder", names(tool)),
    (archived, "pipeline", "old", "Old flow", names(tool)),
    (unrelated, "instruction", "style", "Style", names(other)),
    (pipeline, "pipeline", "delivery", "Delivery", json!({"steps":[]})),
  ] {
    let (status, created) = team.resources("POST", "", &named(id, kind, slug, name, &body));
    assert_eq!(status, 201, "{created}");
  }
  // The pipeline names the tool only in its latest version; the archived one is left out.
  let (status, published) = team.resources("POST", &format!("/{pipeline}/versions"), &publish(1, &names(tool)));
  assert_eq!(status, 201, "{published}");
  let patch = json!({"idempotency_key":IdempotencyKey::generate(),"revision":1,"archived":true});
  assert_eq!(team.resources("PATCH", &format!("/{archived}"), &patch).0, 200);
  team.server.pin_run("kontur", pin("KON-12", 2, false, &[tool]));
  team.server.pin_run("kontur", pin("KON-3", 1, false, &[other, tool]));
  team.server.pin_run("kontur", pin("KON-5", 1, true, &[tool]));
  team.server.pin_run("kontur", pin("KON-9", 1, false, &[other]));

  team.server.stand_as(Standing::Member);
  let (status, used) = team.resources("GET", &format!("/{tool}/used-by"), &Value::Null);
  assert_eq!(status, 200, "{used}");
  let used: UsedBy = serde_json::from_value(used).expect("used by");
  let found: Vec<_> = used
    .resources
    .iter()
    .map(|resource| (resource.kind, resource.name.as_str(), resource.version.get()))
    .collect();
  assert_eq!(
    found,
    [
      (SharedResourceKind::AgentProfile, "Coder", 1),
      (SharedResourceKind::Pipeline, "Delivery", 2)
    ]
  );
  let runs: Vec<_> = used
    .runs
    .iter()
    .map(|run| (run.task_key.as_str(), run.number))
    .collect();
  assert_eq!(runs, [("KON-12", 2), ("KON-3", 1)]);
  assert_eq!(used.runs[0].holder.display_name, "Dana Ruiz");

  let (_, nothing) = team.resources("GET", &format!("/{pipeline}/used-by"), &Value::Null);
  assert_eq!(nothing, json!({"resources": [], "runs": []}));
  let (status, missing) = team.resources("GET", &format!("/{}/used-by", ResourceId::generate()), &Value::Null);
  assert_eq!((status, missing["error"].as_str()), (404, Some("not_found")));
}
