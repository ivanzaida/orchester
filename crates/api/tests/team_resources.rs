//! The stand-in team server keeps an MCP server resource: it takes the kind through create, publish, rename and
//! archive, serves its versions, refuses a body with a path, a value or a local id, and computes `widens`; and it
//! answers used-by from its own resources and the runs a test pins.

use std::{
  io::{Read, Write},
  net::TcpStream,
};

use orchester_api::{
  ResourceCreated, Resources, SharedResource, SharedResourceKind, SharedVersion, UsedBy, Versions,
  test_support::team_server::{CODE, Person, PinnedRun, RunReport, Standing, TeamServer},
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

fn created(id: ResourceId, kind: &str, (slug, name): (&str, &str), version: (&Value, &Value)) -> Value {
  json!({"idempotency_key":IdempotencyKey::generate(),"id":id,"kind":kind,"slug":slug,"name":name,
    "version":{"body":version.0,"references":version.1}})
}

fn tool_reference(id: ResourceId) -> Value {
  json!([{"kind":"tool","id":id,"name":"Deploy","role":null}])
}

fn pipeline_of(kinds: &[Value]) -> Value {
  let steps: Vec<Value> = kinds
    .iter()
    .enumerate()
    .map(|(place, kind)| {
      json!({"key":format!("step-{place}"),"name":"Step","kind":kind,"instructions":"Do it","depends_on":[],
        "approval_gate":false,"on_failure":null,"budget":null})
    })
    .collect();
  json!({"steps":steps})
}

fn agent_step(profile: ResourceId) -> Value {
  json!({"agent":{"profile":profile,"repository":"write"}})
}

fn instruction(text: &str, targets: &Value, scope: &Value) -> Value {
  json!({"text":text,"targets":targets,"events":["start"],"scope":scope})
}

fn pin(task_key: &str, last_report: RunReport, pins: &[ResourceId]) -> PinnedRun {
  PinnedRun {
    task_key: task_key.to_owned(),
    run: RunId::generate(),
    work: 1,
    holder: Some(Person::Named("Dana Ruiz")),
    last_report,
    pins: pins.iter().map(|id| (*id, VersionNumber::FIRST)).collect(),
  }
}

fn used_by(team: &Team, id: impl std::fmt::Display) -> (u16, Value) {
  team.resources("GET", &format!("/{id}/used-by"), &Value::Null)
}

fn create_all(team: &Team, resources: &[Value]) {
  for resource in resources {
    let (status, answer) = team.resources("POST", "", resource);
    assert_eq!(status, 201, "{answer}");
  }
}

fn listed(used: &UsedBy) -> Vec<(SharedResourceKind, &str, u32)> {
  used
    .resources
    .iter()
    .map(|resource| (resource.kind, resource.name.as_str(), resource.version.get()))
    .collect()
}

#[test]
fn stand_in_answers_used_by_with_the_latest_versions_whose_typed_fields_name_the_resource() {
  let team = Team::start(Standing::Admin);
  let [tool, profile, delivery, referencing, archived, decoy, style, own] = [(); 8].map(|()| ResourceId::generate());
  let (empty, none) = (json!({"steps":[]}), json!([]));
  let project = json!("project");
  create_all(
    &team,
    &[
      created(tool, "tool", ("deploy", "Deploy"), (&json!({}), &tool_reference(tool))),
      created(profile, "agent_profile", ("coder", "Coder"), (&json!({}), &none)),
      created(delivery, "pipeline", ("delivery", "Delivery"), (&empty, &none)),
      created(
        referencing,
        "pipeline",
        ("alpha", "alpha flow"),
        (&empty, &tool_reference(tool)),
      ),
      created(
        archived,
        "pipeline",
        ("old", "Old flow"),
        (&pipeline_of(&[agent_step(profile)]), &none),
      ),
      created(
        decoy,
        "instruction",
        ("decoy", "Decoy"),
        (
          &instruction(&format!("call {tool} and {profile}"), &json!("all"), &project),
          &none,
        ),
      ),
      created(
        style,
        "instruction",
        ("style", "Style"),
        (
          &instruction(
            "Be brief",
            &json!({"profiles":[profile]}),
            &json!({"pipeline":{"pipeline":delivery}}),
          ),
          &none,
        ),
      ),
      created(own, "tool", ("own", "Own"), (&json!({}), &tool_reference(own))),
    ],
  );
  // Only the latest version counts: the pipeline names the profile and the tool in its second version.
  let latest = pipeline_of(&[agent_step(profile), json!({"tool":{"tool":tool}})]);
  let (status, published) = team.resources("POST", &format!("/{delivery}/versions"), &publish(1, &latest));
  assert_eq!(status, 201, "{published}");
  let patch = json!({"idempotency_key":IdempotencyKey::generate(),"revision":1,"archived":true});
  assert_eq!(team.resources("PATCH", &format!("/{archived}"), &patch).0, 200);

  team.server.stand_as(Standing::Member);
  let (status, used) = used_by(&team, tool);
  assert_eq!(status, 200, "{used}");
  let used: UsedBy = serde_json::from_value(used).expect("used by");
  // By name without regard to case ("alpha flow" before "Delivery"); the tool is not in its own list, the archived
  // pipeline is left out and an id in an instruction's text names nothing.
  assert_eq!(
    listed(&used),
    [
      (SharedResourceKind::Pipeline, "alpha flow", 1),
      (SharedResourceKind::Pipeline, "Delivery", 2)
    ]
  );
  let (_, used) = used_by(&team, profile);
  let used: UsedBy = serde_json::from_value(used).expect("used by");
  assert_eq!(
    listed(&used),
    [
      (SharedResourceKind::Pipeline, "Delivery", 2),
      (SharedResourceKind::Instruction, "Style", 1)
    ]
  );
  let (_, used) = used_by(&team, delivery);
  let used: UsedBy = serde_json::from_value(used).expect("used by");
  assert_eq!(listed(&used), [(SharedResourceKind::Instruction, "Style", 1)]);
  let (_, nothing) = used_by(&team, own);
  assert_eq!(nothing, json!({"resources": [], "runs": []}));
}

/// A tool "Deploy" in a team that has listed no run yet.
fn team_with_tool() -> (Team, ResourceId) {
  let team = Team::start(Standing::Admin);
  let tool = ResourceId::generate();
  create_all(
    &team,
    &[created(tool, "tool", ("deploy", "Deploy"), (&json!({}), &json!([])))],
  );
  (team, tool)
}

fn listed_runs(team: &Team, resource: ResourceId) -> Vec<RunId> {
  let (status, used) = used_by(team, resource);
  assert_eq!(status, 200, "{used}");
  let used: UsedBy = serde_json::from_value(used).expect("used by");
  assert!(
    used.runs.iter().all(|run| run.holder.is_some()),
    "the team server sets the holder of every run it lists: {used:?}"
  );
  used.runs.iter().map(|run| run.run).collect()
}

#[test]
fn stand_in_lists_the_runs_that_have_not_ended_by_task_number_and_names_the_current_holder() {
  let (team, tool) = team_with_tool();
  let other = ResourceId::generate();
  let running = pin("KON-12", RunReport::Progress, &[tool]);
  let held = pin("KON-3", RunReport::Progress, &[tool, other]);
  let held_run = held.run;
  for run in [running, held] {
    team.server.pin_run("kontur", run);
  }
  for (number, ended) in [
    (5, RunReport::Succeeded),
    (6, RunReport::Failed),
    (7, RunReport::Cancelled),
  ] {
    team
      .server
      .pin_run("kontur", pin(&format!("KON-{number}"), ended, &[tool]));
  }
  team
    .server
    .pin_run("kontur", pin("KON-9", RunReport::Progress, &[other]));
  team
    .server
    .claim_run_task("kontur", held_run, Some(Person::Named("Lena Moroz")));

  team.server.stand_as(Standing::Member);
  let (status, used) = used_by(&team, tool);
  assert_eq!(status, 200, "{used}");
  assert!(used["runs"][0].get("number").is_none(), "a run has no number: {used}");
  let used: UsedBy = serde_json::from_value(used).expect("used by");
  let runs: Vec<_> = used
    .runs
    .iter()
    .map(|run| {
      (
        run.task_key.as_str(),
        run.holder.as_ref().map(|holder| holder.display_name.as_str()),
      )
    })
    .collect();
  assert_eq!(
    runs,
    [("KON-3", Some("Lena Moroz")), ("KON-12", Some("Dana Ruiz"))],
    "every listed run has the holder; the ended runs and the run of another resource are left out"
  );
}

#[test]
fn stand_in_ends_a_run_its_work_superseded_and_orders_the_runs_of_different_tasks_by_task_number() {
  let (team, tool) = team_with_tool();
  // The first run of work 1 last reported `progress`, and a later run of that work was first reported: it has ended.
  let superseded = pin("KON-3", RunReport::Progress, &[tool]);
  let current = pin("KON-3", RunReport::Progress, &[tool]);
  let other_task = pin("KON-2", RunReport::Progress, &[tool]);
  let expected = vec![other_task.run, current.run];
  // KON-2 is pinned after KON-3, so the answer's order is the stand-in's sort, not the order the runs came in.
  for run in [superseded, current, other_task] {
    team.server.pin_run("kontur", run);
  }

  assert_eq!(
    listed_runs(&team, tool),
    expected,
    "KON-3's only live run is the one first reported last in its work; KON-2 comes before KON-3"
  );
}

#[test]
fn stand_in_makes_the_run_whose_first_report_came_last_the_newest_whatever_its_id() {
  let (team, tool) = team_with_tool();
  let mut runs = [
    pin("KON-3", RunReport::Progress, &[tool]),
    pin("KON-3", RunReport::Progress, &[tool]),
  ];
  runs.sort_by_key(|run| run.run);
  let [lower, higher] = runs;
  let lower_run = lower.run;
  // The run with the higher id reported first; the lower id's first report arrives late, so it is the newest.
  team.server.pin_run("kontur", higher);
  team.server.pin_run("kontur", lower);

  assert_eq!(listed_runs(&team, tool), [lower_run]);
}

#[test]
fn stand_in_does_not_list_the_runs_of_a_work_fenced_by_a_take_over() {
  let (team, tool) = team_with_tool();
  let first = pin("KON-3", RunReport::Progress, &[tool]);
  let first_run = first.run;
  team.server.pin_run("kontur", first);
  assert_eq!(
    listed_runs(&team, tool),
    [first_run],
    "the work under the claim is current"
  );

  // Lena takes the task over: the server starts work 2 and fences work 1, whose run last reported `progress`.
  team
    .server
    .claim_run_task("kontur", first_run, Some(Person::Named("Lena Moroz")));
  let mut taken = pin("KON-3", RunReport::Progress, &[tool]);
  taken.work = 2;
  taken.holder = Some(Person::Named("Lena Moroz"));
  let taken_run = taken.run;
  team.server.pin_run("kontur", taken);

  assert_eq!(
    listed_runs(&team, tool),
    [taken_run],
    "the fenced run of work 1 can no longer report, so it is not listed"
  );
}

#[test]
fn stand_in_lists_no_run_of_a_task_whose_claim_was_released() {
  let (team, tool) = team_with_tool();
  let released = pin("KON-4", RunReport::Progress, &[tool]);
  let released_run = released.run;
  team.server.pin_run("kontur", released);
  let kept = pin("KON-5", RunReport::Progress, &[tool]);
  let kept_run = kept.run;
  team.server.pin_run("kontur", kept);
  assert_eq!(listed_runs(&team, tool).len(), 2);

  team.server.claim_run_task("kontur", released_run, None);

  assert_eq!(
    listed_runs(&team, tool),
    [kept_run],
    "a released task has no current work"
  );
}

#[test]
fn stand_in_lists_the_runs_of_a_team_mcp_server_that_its_bundles_pin() {
  let team = Team::start(Standing::Admin);
  let (server, other) = (ResourceId::generate(), ResourceId::generate());
  let (status, created) = team.resources("POST", "", &create(server, &server_body("npx")));
  assert_eq!(status, 201, "{created}");
  team
    .server
    .pin_run("kontur", pin("KON-7", RunReport::Progress, &[server, other]));
  team
    .server
    .pin_run("kontur", pin("KON-8", RunReport::Progress, &[other]));
  team
    .server
    .pin_run("kontur", pin("KON-9", RunReport::Succeeded, &[server]));

  let (status, used) = used_by(&team, server);
  assert_eq!(status, 200, "{used}");
  let used: UsedBy = serde_json::from_value(used).expect("used by");
  assert_eq!(
    used.runs.iter().map(|run| run.task_key.as_str()).collect::<Vec<_>>(),
    ["KON-7"],
    "a run uses a team MCP server through the bundle that pins it until it has ended"
  );
}

#[test]
fn stand_in_answers_used_by_for_an_archived_resource_and_not_found_for_an_unknown_one() {
  let team = Team::start(Standing::Admin);
  let (tool, pipeline) = (ResourceId::generate(), ResourceId::generate());
  let none = json!([]);
  create_all(
    &team,
    &[
      created(tool, "tool", ("deploy", "Deploy"), (&json!({}), &none)),
      created(
        pipeline,
        "pipeline",
        ("flow", "Flow"),
        (&pipeline_of(&[json!({"tool":{"tool":tool}})]), &none),
      ),
    ],
  );
  team
    .server
    .pin_run("kontur", pin("KON-1", RunReport::Progress, &[tool]));
  let patch = json!({"idempotency_key":IdempotencyKey::generate(),"revision":1,"archived":true});
  assert_eq!(team.resources("PATCH", &format!("/{tool}"), &patch).0, 200);

  let (status, used) = used_by(&team, tool);
  assert_eq!(status, 200, "{used}");
  let used: UsedBy = serde_json::from_value(used).expect("used by");
  assert_eq!(listed(&used), [(SharedResourceKind::Pipeline, "Flow", 1)]);
  assert_eq!(used.runs.len(), 1);
  let (status, missing) = used_by(&team, ResourceId::generate());
  assert_eq!((status, missing["error"].as_str()), (404, Some("not_found")));
  let (status, malformed) = used_by(&team, "not-a-resource-id");
  assert_eq!((status, malformed["error"].as_str()), (404, Some("not_found")));
}
