//! The connection a team reviewer runs on, picked over MCP: a language reviewer's of its CLI kind, a decision
//! reviewer's among the decision connections its destinations allow, a hosted one a person's to choose, and a version
//! in the earlier form refused as not the team's form.

use orchester_core::{
  connection::{DecisionConnection, DecisionServerUrl},
  id::{DecisionConnectionId, ResourceId},
  resource::{ResourceBody, TeamReviewer},
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::team_fill_ins::{Team, assert_person_only, name, team, team_resource};

#[test]
fn domain_team_pick_reviewer_connection_keeps_the_pick_for_a_team_reviewer() {
  let mut team = team();
  let connection = team.codex_connection();
  let language = |cli: &str| -> TeamReviewer {
    serde_json::from_value(json!({"kind": "language", "cli": cli, "model": "m-1", "effort": null})).expect("language")
  };
  let decision = |destinations: Value| -> TeamReviewer {
    serde_json::from_value(
      json!({"kind": "decision", "model": "d-1", "lowest_threshold": 0.8, "destinations": destinations}),
    )
    .expect("decision")
  };
  let mut db = team.db();
  let codex = team_resource(&mut db, "codex-reviewer", language("codex_cli"), Vec::new());
  let claude = team_resource(&mut db, "claude-reviewer", language("claude_code_cli"), Vec::new());
  let listing = json!({"kind": "hosts", "hosts": ["api.typesafe.ai"]});
  let decisions = team_resource(&mut db, "decisions", decision(listing), Vec::new());
  let members_only = json!({"kind": "members_computers"});
  let members = team_resource(&mut db, "members", decision(members_only), Vec::new());
  let old = team_resource(&mut db, "old", language("codex_cli"), Vec::new());
  make_old_form(&team, old);
  let (local, hosted) = (
    decision_connection(&mut db, "local", "http://127.0.0.1:8000"),
    decision_connection(&mut db, "hosted", "https://api.typesafe.ai/v1"),
  );
  drop(db);
  team.keep_team();
  let args = |reviewer: ResourceId, connection: &str| json!({ "project": "Kontur", "reviewer": reviewer.to_string(), "connection": connection });

  let picked = team.served.ok("pick_reviewer_connection", &args(codex, &connection));
  assert_eq!(picked["reviewer"]["name"], "codex-reviewer");
  assert_eq!(picked["connection"]["id"], connection);
  assert_eq!(
    team
      .db()
      .connection_pick(codex)
      .expect("read")
      .map(|pick| pick.connection.to_string()),
    Some(connection.clone())
  );
  let refused = team
    .served
    .refused("pick_reviewer_connection", &args(claude, &connection));
  assert_eq!(refused["code"], "connection_does_not_fit", "{refused}");
  assert_eq!(team.db().connection_pick(claude).expect("read"), None);
  let refused = team.served.refused(
    "pick_reviewer_connection",
    &json!({"project": "Kontur", "reviewer": "nobody", "connection": connection}),
  );
  assert_eq!(refused["code"], "reviewer_not_found", "{refused}");

  team
    .served
    .ok("pick_reviewer_connection", &args(decisions, &local.to_string()));
  assert_eq!(team.db().decision_pick(decisions).expect("read"), Some(local));
  let refused = team
    .served
    .refused("pick_reviewer_connection", &args(decisions, &hosted.to_string()));
  assert_person_only(&refused);
  assert_eq!(
    team.db().decision_pick(decisions).expect("read"),
    Some(local),
    "the pick stays"
  );
  let refused = team
    .served
    .refused("pick_reviewer_connection", &args(decisions, &connection));
  assert_eq!(refused["code"], "connection_not_found", "{refused}");

  let refused = team
    .served
    .refused("pick_reviewer_connection", &args(members, &hosted.to_string()));
  assert_eq!(refused["code"], "server_not_allowed", "{refused}");
  assert_eq!(team.db().decision_pick(members).expect("read"), None);
  let refused = team.served.refused("pick_reviewer_connection", &args(old, &connection));
  assert_eq!(refused["code"], "not_team_form", "{refused}");
  assert_eq!(team.db().connection_pick(old).expect("read"), None);
}

/// Writes the body of the earlier form (it names a connection) over the newest version of `reviewer`, as the server
/// sends a version published before ORC-887 and a cache from before it holds it.
fn make_old_form(team: &Team, reviewer: ResourceId) {
  let old = json!({
    "kind": "language", "connection": {"kind": "connection", "name": "Team language", "role": "codex_cli"},
    "model": "m-1", "effort": null
  });
  let stored = json!({"schema": <TeamReviewer as ResourceBody>::SCHEMA, "body": old}).to_string();
  let database = rusqlite::Connection::open(team.repo.join(".orchester").join("orchester.db")).expect("database");
  let changed = database
    .execute(
      "UPDATE shared_version_bodies SET local_body = ?1, server_body = ?2 \
       WHERE version_id IN (SELECT id FROM resource_versions WHERE resource_id = ?3)",
      rusqlite::params![stored, old.to_string(), reviewer.to_string()],
    )
    .expect("the old form");
  assert_eq!(changed, 1);
}

fn decision_connection(db: &mut ProjectDb, label: &str, url: &str) -> DecisionConnectionId {
  let made = DecisionConnection::new(
    DecisionConnectionId::generate(),
    name(label),
    DecisionServerUrl::new(url).expect("server"),
    None,
  );
  db.create_decision_connection(&made).expect("a decision connection");
  made.id()
}
