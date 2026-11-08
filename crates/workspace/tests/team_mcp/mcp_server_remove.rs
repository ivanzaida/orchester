//! Removing a team MCP server against the fake team server of [`super::mcp_servers`]: the tool reads what uses the
//! server, archives it on the server, and reports the runs of this computer that will stop, without claiming they
//! stopped; it archives nothing while what uses it cannot be read, and refuses a server the team server has not
//! received yet.

use std::{collections::BTreeMap, time::Duration};

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, IdempotencyKey, ResourceId, RunId, TaskId, UserId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, ResourceKind, Slug, Step, StepKey,
    StepKind,
  },
  revision::Revision,
  run::{Answerer, BranchName, NewRun, PublishedVersions, RunSnapshot},
  sharing::SharedPerson,
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::{CachedVersion, ProjectDb, ReferenceTarget, ResourceSharing, VersionReference};
use orchester_workspace::sharing::NotArchived;
use serde_json::json;

use super::{
  created::admin,
  mcp_servers::{Answers, docs, start},
};
use crate::{Checkout, Standing};

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
fn team_mcp_a_remove_of_a_server_the_team_server_has_not_received_says_so() {
  let (server, seen) = start(Answers::default());
  let checkout = admin(&server);
  server.set_down(true);
  checkout.call("add_mcp_server", &docs()).expect("kept here, unsent");
  server.set_down(false);

  let refused = checkout
    .call("remove_mcp_server", &json!({ "project": "Kontur", "server": "docs" }))
    .expect_err("refused");

  assert_eq!(refused["error"]["code"], "team_mcp_server_not_sent", "{refused}");
  let updated = json!({ "project": "Kontur", "server": "docs", "args": ["--other"] });
  assert_eq!(
    checkout.refused("update_mcp_server", &updated),
    "team_mcp_server_not_sent"
  );
  assert!(
    seen.lock().calls.is_empty(),
    "the team server was not asked about a server it never saw"
  );
  assert_eq!(
    checkout.db().resources(ResourceKind::McpServer).expect("servers").len(),
    1
  );
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
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(2))
    .expect("profile")
    .id();
  let published = db.publish_draft::<AgentProfile>(builder, at(3)).expect("publish");
  // The team's version of the profile, as the server confirmed it: it chooses the server by a typed reference, which
  // is what the rule for the runs an archive stops reads.
  db.set_resource_sharing(builder, Some(ResourceSharing::Shared(revision)))
    .expect("the profile is the team's");
  let confirmed = CachedVersion {
    number: published.number(),
    published_at: at(3),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Olena".to_owned(),
    },
    server_body: json!({}),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references: vec![VersionReference {
      target: ReferenceTarget::TeamResource {
        kind: ResourceKind::McpServer,
        id: server,
      },
      name: String::new(),
      role: None,
    }],
    local_body: published.body().clone(),
    waits_here: false,
  };
  db.cache_shared_version(builder, &confirmed)
    .expect("the team version is cached");
  let profiles = BTreeMap::from([(builder, published)]);
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

#[test]
fn team_mcp_an_archive_asked_by_automation_is_refused_before_anything_is_sent() {
  let (server, seen) = start(Answers {
    used_by: Some(json!({ "resources": [], "runs": [] })),
    ..Answers::default()
  });
  let checkout = admin(&server);
  checkout.call("add_mcp_server", &docs()).expect("created");
  let id = checkout.db().resources(ResourceKind::McpServer).expect("servers")[0].id();

  let refused = checkout
    .runtime
    .block_on(
      checkout
        .servers
        .archive_resource(checkout.root(), id, Answerer::Automation),
    )
    .expect_err("a person's alone");

  assert_eq!(refused, NotArchived::PersonOnly);
  assert_eq!(seen.lock().calls, ["create"], "nothing was read or archived");
  let still = checkout.db().resources(ResourceKind::McpServer).expect("servers");
  assert_eq!(still.len(), 1, "the server is as it was");
}
