//! A team reviewer's connection, like a profile's: by kind when there is one, by the person's pick when there are
//! several. A reviewer cached before ORC-887 names a connection and is not read.

use orchester_core::{
  connection::{DecisionConnection, DecisionServerUrl},
  id::DecisionConnectionId,
  resource::{ResourceBody, TeamReviewer},
  sharing::{ConnectionProblem, Missing},
};
use orchester_workspace::readiness::{Group, ReadinessStanding, Undecided};
use serde_json::{Value, json};

use crate::support::{Fixture, name};

fn language(cli: &str) -> TeamReviewer {
  serde_json::from_value(json!({"kind": "language", "cli": cli, "model": "m-1", "effort": null})).expect("language")
}

fn decision() -> TeamReviewer {
  serde_json::from_value(json!({
    "kind": "decision", "model": "d-1", "lowest_threshold": 0.8, "destinations": {"kind": "members_computers"}
  }))
  .expect("decision")
}

fn decision_connection(fixture: &Fixture, label: &str, url: &str) -> DecisionConnectionId {
  let made = DecisionConnection::new(
    DecisionConnectionId::generate(),
    name(label),
    DecisionServerUrl::new(url).expect("server"),
    None,
  );
  fixture
    .db()
    .create_decision_connection(&made)
    .expect("a decision connection");
  made.id()
}

#[test]
fn readiness_a_language_reviewer_needs_a_connection_of_its_cli_kind_and_a_pick_among_several() {
  let fixture = Fixture::team();
  let id = fixture.team_resource("reviewer", language("claude_code_cli"));
  fixture.connection(orchester_core::connection::AgentProvider::CodexCli, "codex");

  let read = fixture.read();
  let found = read.resource(id).expect("listed");
  assert_eq!(found.group(), Group::NeedsYouHere);
  assert_eq!(
    found.standing().first(),
    Some(&Missing::Connection {
      profile: id,
      cli: orchester_core::connection::AgentProvider::ClaudeCodeCli,
      problem: ConnectionProblem::NoneOfKind
    }),
    "a connection of another kind does not count"
  );

  let work = fixture.connection(orchester_core::connection::AgentProvider::ClaudeCodeCli, "work");
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
  fixture.connection(orchester_core::connection::AgentProvider::ClaudeCodeCli, "personal");
  assert!(matches!(
    fixture.read().resource(id).expect("listed").standing().first(),
    Some(Missing::Connection {
      problem: ConnectionProblem::PickNeeded,
      ..
    })
  ));
  fixture
    .workspace
    .pick_team_reviewer_connection(&fixture.root, id, work)
    .expect("pick");
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}

#[test]
fn readiness_a_decision_reviewer_needs_a_decision_connection() {
  let fixture = Fixture::team();
  let id = fixture.team_resource("decisions", decision());
  fixture.connection(orchester_core::connection::AgentProvider::ClaudeCodeCli, "claude");

  assert_eq!(
    fixture.read().resource(id).expect("listed").standing().first(),
    Some(&Missing::DecisionConnection {
      reviewer: id,
      problem: ConnectionProblem::NoneOfKind
    }),
    "an agent connection is not a decision connection"
  );
  decision_connection(&fixture, "local", "http://127.0.0.1:8000");
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}

#[test]
fn readiness_a_reviewer_that_names_a_connection_is_not_read_and_says_so() {
  let fixture = Fixture::team();
  let id = fixture.team_resource("old", language("claude_code_cli"));
  let old: Value = json!({
    "kind": "language", "connection": {"kind": "connection", "name": "Team language", "role": "claude_code_cli"},
    "model": "m-1", "effort": null
  });
  let stored = json!({"schema": <TeamReviewer as ResourceBody>::SCHEMA, "body": old}).to_string();
  let database = rusqlite::Connection::open(fixture.root.join(".orchester").join("orchester.db")).expect("database");
  let changed = database
    .execute(
      "UPDATE shared_version_bodies SET local_body = ?1 \
       WHERE version_id IN (SELECT id FROM resource_versions WHERE resource_id = ?2)",
      rusqlite::params![stored, id.to_string()],
    )
    .expect("the old form");
  assert_eq!(changed, 1);
  drop(database);

  let read = fixture.read();
  let found = read.resource(id).expect("listed");
  assert_eq!(found.standing(), &ReadinessStanding::Undecided(Undecided::NotTeamForm));
  assert_eq!(found.group(), Group::NeedsYouHere);
}
