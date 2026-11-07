//! A team task's pipeline through the tools (`docs/architecture/server.md` row 80, the person's decision of
//! 2026-11-05): `create_task` and `update_task` send one of the team's pipelines by its id and refuse a pipeline of
//! this computer's own; `run_task` runs only the team pipeline a task names; and a team project offers no pipeline of
//! this computer's own (`list_pipelines`, `get_pipeline`).

use orchester_core::{
  blob::ContentHash,
  id::{ResourceId, UserId},
  name::Name,
  resource::{Pipeline, Resource, ResourceKind, Slug, VersionNumber},
  sharing::{SharedPerson, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb};
use serde_json::json;

use crate::{Checkout, tools::server};

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

/// Keeps `id` as one of the team's pipelines, named `slug`, with version 1 as the server confirmed it.
pub(crate) fn team_pipeline(db: &mut ProjectDb, id: ResourceId, slug: &str) {
  let now = Timestamp::now();
  let resource = Resource::new(
    id,
    ResourceKind::Pipeline,
    Slug::new(slug).expect("slug"),
    name(slug),
    now,
  );
  let revision = TaskRevision::try_from(1).expect("a revision");
  db.keep_shared_resource(&resource, revision)
    .expect("the team's pipeline");
  cache(
    db,
    id,
    crate::fixture::one_step(ResourceId::generate(), "Say hello.").body(),
  );
}

fn cache(db: &mut ProjectDb, id: ResourceId, body: &Pipeline) {
  let version = CachedVersion {
    number: VersionNumber::FIRST,
    published_at: Timestamp::now(),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Dana".to_owned(),
    },
    server_body: serde_json::to_value(body).expect("a body"),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references: Vec::new(),
    local_body: body.clone(),
    waits_here: false,
  };
  db.cache_shared_version(id, &version).expect("its version");
}

/// A pipeline of this computer's own named `slug`, published here; with `id` when given.
fn own_pipeline(db: &mut ProjectDb, slug: &str, id: Option<ResourceId>) -> ResourceId {
  let Some(id) = id else {
    let body = crate::fixture::one_step(ResourceId::generate(), "Say hello.");
    let created = db
      .create_resource(
        Slug::new(slug).expect("slug"),
        name(slug),
        body.body(),
        Timestamp::now(),
      )
      .expect("a pipeline")
      .id();
    db.publish_draft::<Pipeline>(created, Timestamp::now())
      .expect("published");
    return created;
  };
  team_pipeline(db, id, slug);
  db.set_resource_sharing(id, None).expect("kept on this computer only");
  id
}

#[test]
fn team_mcp_create_and_update_name_a_team_pipeline_by_its_id() {
  let (server, seen) = server();
  let delivery = server.state().config.pipeline;
  let review = ResourceId::generate();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    team_pipeline(db, delivery, "delivery");
    team_pipeline(db, review, "review");
    own_pipeline(db, "own-delivery", None);
  });

  // A member reaches the server's task.create, which is the Admins': the pipeline went with it, by its id.
  let create = json!({"project": "Kontur", "title": "Say goodbye", "pipeline": "delivery"});
  assert_eq!(checkout.refused("create_task", &create), "not_admin");
  assert_eq!(seen.lock().created_pipeline, json!(delivery.to_string()));

  // The same pipeline again sends no pipeline change; another one is a person's decision.
  let same = json!({"project": "Kontur", "key": "KON-7", "pipeline": delivery.to_string(), "title": "Say goodbye"});
  assert_eq!(checkout.refused("update_task", &same), "not_admin");
  assert_eq!(seen.lock().edited_pipeline, json!(null), "no pipeline change sent");
  let other = json!({"project": "Kontur", "key": "KON-7", "pipeline": "review"});
  assert_eq!(checkout.refused("update_task", &other), "person_only");
  assert_eq!(seen.lock().task_edits, 1, "the change of pipeline was not sent");
}

#[test]
fn team_mcp_a_pipeline_of_this_computer_and_limits_are_refused() {
  let (server, seen) = server();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    own_pipeline(db, "own-delivery", None);
  });

  for named in ["own-delivery", "nothing-like-it"] {
    let create = json!({"project": "Kontur", "title": "Say goodbye", "pipeline": named});
    assert_eq!(checkout.refused("create_task", &create), "team_pipeline_required");
    let update = json!({"project": "Kontur", "key": "KON-7", "pipeline": named});
    assert_eq!(checkout.refused("update_task", &update), "team_pipeline_required");
  }
  let budget = json!({"project": "Kontur", "title": "Say goodbye", "budget_tokens": 1000});
  assert_eq!(checkout.refused("create_task", &budget), "decision_required");
  let seen = seen.lock();
  assert_eq!((seen.task_creates, seen.task_edits), (0, 0), "nothing was sent");
}

#[test]
fn team_mcp_run_task_runs_only_the_team_pipeline_a_task_names() {
  let (server, _seen) = server();
  let named = server.state().config.pipeline;
  // This computer has the task's pipeline id only as its own: it is no team pipeline here.
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    own_pipeline(db, "delivery", Some(named));
  });
  let run = json!({"project": "Kontur", "key": "KON-7"});
  assert_eq!(checkout.refused("run_task", &run), "team_pipeline_missing");
  let unknown = Checkout::new(Some(&server), &[]);
  assert_eq!(unknown.refused("run_task", &run), "team_pipeline_missing");
  let state = server.state();
  assert_eq!(state.works_made, 0, "no work started");
  assert!(state.claim.is_none(), "nothing claimed");
  drop(state);
  assert!(
    matches!(checkout.db().resource_sharing(named), Ok(None)),
    "the pipeline of this computer's own stays as it is"
  );
}

#[test]
fn team_mcp_a_team_project_offers_no_pipeline_of_this_computers_own() {
  let (server, _seen) = server();
  let delivery = server.state().config.pipeline;
  let mut own = None;
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    team_pipeline(db, delivery, "delivery");
    own = Some(own_pipeline(db, "own-delivery", None));
  });
  let own = own.expect("made");

  let listed = checkout
    .call("list_pipelines", &json!({"project": "Kontur"}))
    .expect("listed");
  let ids: Vec<&str> = listed["pipelines"]
    .as_array()
    .expect("pipelines")
    .iter()
    .filter_map(|pipeline| pipeline["id"].as_str())
    .collect();
  assert_eq!(ids, [delivery.to_string()]);
  let opened = json!({"project": "Kontur", "pipeline": own.to_string()});
  assert_eq!(checkout.refused("get_pipeline", &opened), "pipeline_not_found");
  assert!(
    checkout.db().resource(own).expect("read").is_some(),
    "kept in the project's database as it is"
  );
  assert!(matches!(checkout.db().resource_sharing(own), Ok(None)));
}

/// A profile's body names its connection by the publisher's id and its version the CLI by a `CliKind` reference; when
/// this computer has that very connection for the CLI, the start binds to it without asking, even among several of
/// that CLI. The team MCP server the version names runs as this computer's server with the same id.
#[test]
fn team_mcp_run_task_binds_a_profile_to_the_connection_its_body_names() {
  let (server, seen) = server();
  let pipeline = server.state().config.pipeline;
  let named = seen.lock().publisher_connection.expect("the Builder's connection");
  let mcp_server = seen.lock().publisher_mcp_server.expect("the Builder's MCP server");
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    team_pipeline(db, pipeline, "delivery");
    orchester_core::store::McpServerStore::create_mcp_server(db, &crate::tools::local_mcp_server(mcp_server))
      .expect("a local MCP server");
    for (id, label) in [
      (named, "Codex here"),
      (orchester_core::id::ConnectionId::generate(), "Codex work"),
    ] {
      let connection = orchester_core::connection::AgentConnection::new(
        id,
        name(label),
        orchester_core::connection::AgentProvider::CodexCli,
        std::env::temp_dir().join("codex.exe"),
        Timestamp::now(),
      )
      .expect("connection");
      orchester_core::store::ConnectionStore::create_connection(db, &connection).expect("a connection");
    }
  });
  let code = checkout.refused("run_task", &json!({"project": "Kontur", "key": "KON-7"}));
  assert_eq!(code, "runner_unavailable", "bound and started; no runner here");
  assert_eq!(server.state().works_made, 1);
}
