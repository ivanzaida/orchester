//! The MCP server tools in a team project against a fake team server: a new server is created as v1 once the server
//! accepts it and taken back when it refuses; an update publishes the next version, which waits in the outbox while the
//! server cannot be reached; a remove reads what uses the server, archives it on the server, and reports the runs of
//! this computer that will stop, without claiming they stopped. A member changes none of it.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use orchester_core::{
  connection::mcp::McpServerSelection,
  id::{ConnectionId, IdempotencyKey, ResourceId, RunId, TaskId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, ResourceKind, Slug, Step, StepKey,
    StepKind, TeamMcpServer,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::{ProjectDb, PublishState};
use parking_lot::Mutex;
use serde_json::{Value, json};

use super::created::{admin, created};
use crate::{
  Checkout, Standing,
  fixture::Config,
  http::{Request, respond},
  team_server::{Extra, TeamServer},
};

/// What the fake server saw, in order, and what each request carried.
#[derive(Default)]
struct Seen {
  calls: Vec<String>,
  created: Vec<Value>,
  published: Vec<Value>,
  patched: Vec<Value>,
}

/// How the fake server answers: `None` accepts, `Some((status, code))` refuses.
#[derive(Default)]
struct Answers {
  create: Option<(u16, &'static str)>,
  publish: Option<(u16, &'static str)>,
  archive: Option<(u16, &'static str)>,
  /// What `used-by` answers; the route is not found when `None`.
  used_by: Option<Value>,
}

fn start(answers: Answers) -> (TeamServer, Arc<Mutex<Seen>>) {
  let seen = Arc::new(Mutex::new(Seen::default()));
  let recorded = seen.clone();
  let answers = Arc::new(answers);
  let extra: Extra = Arc::new(move |state, request, stream| {
    let project = state.lock().project;
    let Some(route) = request.path.split('?').next().and_then(|path| {
      path
        .strip_prefix(&format!("/api/v1/projects/{project}/resources"))
        .map(str::to_owned)
    }) else {
      return false;
    };
    let mut seen = recorded.lock();
    match (request.method.as_str(), route.as_str()) {
      ("POST", "") => {
        seen.calls.push("create".to_owned());
        seen.created.push(request.json());
        reply(stream, answers.create, || created(request));
      }
      ("POST", versions) if versions.ends_with("/versions") => {
        seen.calls.push("publish".to_owned());
        seen.published.push(request.json());
        reply(
          stream,
          answers.publish,
          || json!({ "version": published(request, versions) }),
        );
      }
      ("PATCH", _) => {
        seen.calls.push("archive".to_owned());
        seen.patched.push(request.json());
        reply(stream, answers.archive, || json!({ "resource": archived(&route) }));
      }
      ("GET", used) if used.ends_with("/used-by") => {
        seen.calls.push("used-by".to_owned());
        match &answers.used_by {
          Some(used_by) => respond(stream, 200, used_by),
          None => respond(
            stream,
            404,
            &json!({"error": "not_found", "message": "No such resource."}),
          ),
        }
      }
      _ => return false,
    }
    true
  });
  (TeamServer::start_with(Config::new("Say hello."), Some(extra)), seen)
}

fn reply(stream: &mut std::net::TcpStream, refusal: Option<(u16, &str)>, accepted: impl FnOnce() -> Value) {
  match refusal {
    Some((status, code)) => respond(stream, status, &json!({"error": code, "message": "Refused."})),
    None => respond(stream, 201, &accepted()),
  }
}

fn attribution() -> Value {
  json!({ "by": { "id": "01a183cf-b73c-71db-8a62-bd509770cd4e", "display_name": "Dana" }, "at": 1 })
}

/// The version the server numbers after the one `request` is based on.
fn published(request: &Request, route: &str) -> Value {
  let publish = request.json();
  let resource = route.trim_start_matches('/').split('/').next().unwrap_or_default();
  json!({
    "resource": resource, "number": publish["based_on"].as_u64().unwrap_or_default() + 1, "body": publish["body"],
    "hash": "b".repeat(64), "references": [], "based_on": publish["based_on"], "published": attribution(),
    "widens": false
  })
}

fn archived(route: &str) -> Value {
  let resource = route.trim_start_matches('/');
  json!({
    "id": resource, "kind": "mcp_server", "slug": "docs", "name": "docs", "archived": true, "revision": 2,
    "latest": 1, "created": attribution(), "updated": attribution()
  })
}

fn docs() -> Value {
  json!({
    "project": "Kontur", "name": "docs", "program": "npx", "args": ["-y", "@acme/docs"],
    "variable_names": ["TOKEN"], "exempt_tools": ["search"],
  })
}

fn change(fields: Value) -> Value {
  let mut args = json!({ "project": "Kontur", "server": "docs" });
  if let (Value::Object(args), Value::Object(fields)) = (&mut args, fields) {
    args.extend(fields);
  }
  args
}

fn count(checkout: &Checkout) -> usize {
  checkout
    .db()
    .resources(ResourceKind::McpServer)
    .expect("the servers")
    .len()
}

/// The id of the team MCP server `docs` of the checkout.
fn docs_id(checkout: &Checkout) -> ResourceId {
  checkout
    .db()
    .resources(ResourceKind::McpServer)
    .expect("the servers")
    .remove(0)
    .id()
}

fn versions(checkout: &Checkout) -> usize {
  checkout
    .db()
    .versions::<TeamMcpServer>(docs_id(checkout))
    .expect("versions")
    .len()
}

#[test]
fn team_mcp_a_new_mcp_server_is_created_as_v1_once_the_server_accepts_it() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);

  let made = checkout.call("add_mcp_server", &docs()).expect("created");

  assert_eq!(made["server"]["published_version"], 1, "{made}");
  assert_eq!(made["server"]["team"], true, "{made}");
  assert_eq!(made["server"]["launch"]["variable_names"], json!(["TOKEN"]), "{made}");
  assert!(made["server"].get("waiting_for_server").is_none(), "{made}");
  let seen = seen.lock();
  assert_eq!(seen.calls, ["create"]);
  assert_eq!(seen.created[0]["kind"], "mcp_server");
  assert_eq!(
    seen.created[0]["version"]["body"],
    json!({
      "name": "docs",
      "launch": { "kind": "program", "program": "npx", "args": ["-y", "@acme/docs"], "variables": ["TOKEN"] },
      "exempt_tools": ["search"],
    }),
    "the body holds names only"
  );
  assert_eq!(count(&checkout), 1);
  assert_eq!(versions(&checkout), 1);
  assert!(checkout.db().waiting_publishes().expect("the outbox").is_empty());
  let id = docs_id(&checkout);
  assert!(
    checkout
      .db()
      .draft::<TeamMcpServer>(id)
      .expect("a draft read")
      .is_none()
  );
}

#[test]
fn team_mcp_a_new_mcp_server_the_team_server_refuses_is_not_created() {
  let (server, seen) = start(Answers {
    create: Some((403, "not_admin")),
    ..Answers::default()
  });
  let checkout = admin(&server);

  let refused = checkout.call("add_mcp_server", &docs()).expect_err("refused");

  assert_eq!(refused["error"]["code"], "team_server_refused", "{refused}");
  assert_eq!(seen.lock().calls, ["create"]);
  assert_eq!(count(&checkout), 0, "nothing shows as created");
  assert!(checkout.db().waiting_publishes().expect("the outbox").is_empty());
}

#[test]
fn team_mcp_a_new_mcp_server_made_offline_waits_in_the_outbox() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  server.set_down(true);

  let made = checkout.call("add_mcp_server", &docs()).expect("created");

  assert_eq!(
    made["server"]["waiting_for_server"]["code"], "team_server_unreachable",
    "{made}"
  );
  assert!(seen.lock().calls.is_empty());
  assert_eq!(count(&checkout), 1);
  let waiting = checkout.db().waiting_publishes().expect("the outbox");
  assert_eq!(waiting.len(), 1);
  assert_eq!(waiting[0].state, PublishState::Waiting);
}

#[test]
fn team_mcp_a_member_changes_no_mcp_server() {
  let (server, seen) = start(Answers::default());
  let member = Standing {
    role: "member",
    widens: true,
  };
  let checkout = Checkout::standing(Some(&server), (&[], member), |_| {});

  assert_eq!(checkout.refused("add_mcp_server", &docs()), "not_admin");

  assert!(
    seen.lock().calls.is_empty(),
    "this computer refused before the server was asked"
  );
  assert_eq!(count(&checkout), 0);
}

#[test]
fn team_mcp_an_update_publishes_the_next_version_on_the_team_server() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");

  let same = checkout
    .call("update_mcp_server", &change(json!({ "exempt_tools": ["search"] })))
    .expect("unchanged");
  assert_eq!(same["unchanged"], true, "{same}");
  assert_eq!(
    seen.lock().calls,
    ["create"],
    "a change that changes nothing publishes nothing"
  );

  let changed = checkout
    .call(
      "update_mcp_server",
      &change(json!({ "args": ["-y", "@acme/docs", "--lang", "en"] })),
    )
    .expect("published");

  assert_eq!(changed["server"]["published_version"], 2, "{changed}");
  assert_eq!(
    changed["server"]["launch"]["args"],
    json!(["-y", "@acme/docs", "--lang", "en"]),
    "{changed}"
  );
  assert!(changed["server"].get("waiting_for_server").is_none(), "{changed}");
  let seen = seen.lock();
  assert_eq!(seen.calls, ["create", "publish"]);
  assert_eq!(seen.published[0]["based_on"], 1);
  assert_eq!(
    seen.published[0]["body"]["launch"]["args"],
    json!(["-y", "@acme/docs", "--lang", "en"])
  );
  assert_eq!(versions(&checkout), 2);
  assert!(checkout.db().waiting_publishes().expect("the outbox").is_empty());
}

#[test]
fn team_mcp_an_update_made_offline_waits_and_one_the_server_refuses_stays_unsent() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");

  server.set_down(true);
  let waiting = checkout
    .call("update_mcp_server", &change(json!({ "args": ["--other"] })))
    .expect("kept here");
  assert_eq!(
    waiting["server"]["waiting_for_server"]["code"], "team_server_unreachable",
    "{waiting}"
  );
  assert_eq!(checkout.db().waiting_publishes().expect("the outbox").len(), 1);
  assert_eq!(seen.lock().calls, ["create"]);

  let (server, seen) = start(Answers {
    publish: Some((403, "not_admin")),
    ..Answers::default()
  });
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");
  let refused = checkout
    .call("update_mcp_server", &change(json!({ "args": ["--other"] })))
    .expect_err("refused");
  assert_eq!(refused["error"]["code"], "team_server_refused", "{refused}");
  assert_eq!(seen.lock().calls, ["create", "publish"]);
  let outbox = checkout.db().waiting_publishes().expect("the outbox");
  assert_eq!(
    outbox[0].state,
    PublishState::Refused {
      code: "not_admin".to_owned()
    },
    "the version stays here, unsent, for a person to decide"
  );
}

#[test]
fn team_mcp_a_remove_reads_what_uses_the_server_then_archives_it() {
  let used_by = json!({
    "resources": [{ "kind": "agent_profile", "id": "01a183cf-b73c-71db-8a62-bd509770cd01", "name": "Builder", "version": 3 }],
    "runs": [{ "task_key": "KON-12", "run": "01a183cf-b73c-71db-8a62-bd509770cd02" }],
  });
  let (server, seen) = start(Answers {
    used_by: Some(used_by),
    ..Answers::default()
  });
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");

  let removed = checkout
    .call("remove_mcp_server", &json!({ "project": "Kontur", "server": "docs" }))
    .expect("archived");

  assert_eq!(removed["removed"]["name"], "docs", "{removed}");
  assert_eq!(removed["removed"]["archived"], true, "{removed}");
  assert_eq!(removed["used_by"]["resources"][0]["name"], "Builder", "{removed}");
  assert_eq!(removed["used_by"]["runs"][0]["task_key"], "KON-12", "{removed}");
  assert_eq!(removed["runs_stopping_here"], json!([]), "{removed}");
  let seen = seen.lock();
  assert_eq!(
    seen.calls,
    ["create", "used-by", "archive"],
    "what uses it is read first"
  );
  assert_eq!(seen.patched[0]["archived"], true);
  assert_eq!(seen.patched[0]["revision"], 1);
}

#[test]
fn team_mcp_a_remove_archives_nothing_while_what_uses_it_cannot_be_read_or_the_server_refuses() {
  let nothing = || json!({ "resources": [], "runs": [] });
  let (server, seen) = start(Answers {
    used_by: Some(nothing()),
    archive: Some((403, "not_admin")),
    ..Answers::default()
  });
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");
  let refused = checkout
    .call("remove_mcp_server", &json!({ "project": "Kontur", "server": "docs" }))
    .expect_err("refused");
  assert_eq!(refused["error"]["code"], "team_server_refused", "{refused}");
  assert_eq!(seen.lock().calls, ["create", "used-by", "archive"]);

  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");
  let refused = checkout.refused("remove_mcp_server", &json!({ "project": "Kontur", "server": "docs" }));
  assert_eq!(
    refused, "mcp_server_not_found",
    "the server could not tell what uses it"
  );
  assert_eq!(seen.lock().calls, ["create", "used-by"], "no archive was sent");

  let (server, seen) = start(Answers {
    used_by: Some(nothing()),
    ..Answers::default()
  });
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");
  server.set_down(true);
  assert_eq!(
    checkout.refused("remove_mcp_server", &json!({ "project": "Kontur", "server": "docs" })),
    "team_unavailable"
  );
  assert_eq!(seen.lock().calls, ["create"]);
}

#[test]
fn team_mcp_a_remove_names_the_runs_of_this_computer_that_will_stop_without_claiming_they_stopped() {
  let (server, seen) = start(Answers {
    used_by: Some(json!({ "resources": [], "runs": [] })),
    ..Answers::default()
  });
  let knowledge = ResourceId::generate();
  let mut pinned = None;
  let owner = Standing {
    role: "owner",
    widens: true,
  };
  let checkout = Checkout::build(
    Some(&server),
    (&[], owner),
    |db| pinned = Some(pin_run(db, knowledge)),
    true,
  );
  let (run, task) = pinned.expect("a run");

  let removed = checkout
    .call(
      "remove_mcp_server",
      &json!({ "project": "Kontur", "server": knowledge.to_string() }),
    )
    .expect("archived");

  assert_eq!(
    removed["runs_stopping_here"],
    json!([{ "run": run.to_string(), "task": task.to_string() }]),
    "{removed}"
  );
  assert_eq!(seen.lock().calls, ["used-by", "archive"]);
  let state = checkout
    .db()
    .unfinished_runs()
    .expect("runs")
    .into_iter()
    .flatten()
    .count();
  assert_eq!(
    state, 1,
    "the run is still running: the stream's archive event stops it, not this answer"
  );
}

/// A started run of a task that pins a profile choosing the team MCP server `server`, kept in the project as a synced
/// one is (before the project is linked, so the tasks and runs are this computer's own).
fn pin_run(db: &mut ProjectDb, server: ResourceId) -> (RunId, TaskId) {
  let at = Timestamp::from_unix_millis;
  let name = |text: &str| Name::new(text).expect("valid name");
  let resource = orchester_core::resource::Resource::new(
    server,
    ResourceKind::McpServer,
    Slug::new("knowledge").expect("slug"),
    name("knowledge"),
    at(1),
  );
  let revision = orchester_core::sharing::TaskRevision::try_from(1_u64).expect("a revision");
  db.keep_shared_resource(&resource, revision).expect("kept");
  let chosen = ConnectionId::from_uuid(server.as_uuid());
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(vec![chosen]).expect("servers"));
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(2))
    .expect("profile")
    .id();
  let profiles = BTreeMap::from([(
    builder,
    db.publish_draft::<AgentProfile>(builder, at(3)).expect("publish"),
  )]);
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at(4))
    .expect("pipeline")
    .id();
  let version = db.publish_draft::<Pipeline>(delivery, at(5)).expect("publish");
  let (tools, instructions, skills) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &instructions,
    skills: &skills,
  };
  let snapshot = RunSnapshot::build(delivery, &version, published).expect("snapshot");
  started_run(db, snapshot)
}

/// A started run of a new task on `snapshot`.
fn started_run(db: &mut ProjectDb, snapshot: RunSnapshot) -> (RunId, TaskId) {
  let at = Timestamp::from_unix_millis;
  let name = |text: &str| Name::new(text).expect("valid name");
  let budget = TokenBudget::new(1_000_000).expect("above zero");
  let details = TaskDetails {
    title: name("Search"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::new(Some(budget), Some(Duration::from_secs(1_800))).expect("limits"),
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(10))
    .expect("task")[0]
    .id();
  let new = NewRun {
    task,
    snapshot,
    branch: BranchName::new("orchester/kon-run-1").expect("branch"),
    base: Revision::new(&"a".repeat(40)).expect("revision"),
    base_branch: None,
    base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
    budget: Some(budget),
    max_duration: Some(Duration::from_secs(1_800)),
  };
  let mut run = db.create_run(IdempotencyKey::generate(), new, at(11)).expect("run");
  let events = run.start(at(11)).expect("start");
  db.append_events(&run, &events).expect("append");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, at(11)).expect("start step");
  db.append_events(&run, &events).expect("append");
  (run.id(), task)
}
