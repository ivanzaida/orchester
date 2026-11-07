//! The connection a team reviewer runs on here: an agent connection of its CLI kind for a language reviewer, a decision
//! connection for a decision one, kept per reviewer on this computer.

use std::collections::BTreeSet;

use orchester_core::{
  connection::{DecisionConnection, DecisionServerUrl},
  id::{DecisionConnectionId, ResourceId},
  resource::{ModelId, ResourceBody, ReviewerDestinations, ReviewerHost, TeamReviewer},
  run::Answerer,
};
use orchester_store::ProjectDb;
use orchester_workspace::team_fill_ins::FillInError;

use crate::support::{Team, name, pick, team_resource};

fn language() -> TeamReviewer {
  serde_json::from_value(
    serde_json::json!({"kind": "language", "cli": "claude_code_cli", "model": "m-1", "effort": null}),
  )
  .expect("language")
}

fn decision(destinations: ReviewerDestinations) -> TeamReviewer {
  TeamReviewer::Decision {
    model: ModelId::new("d-1").expect("model"),
    lowest_threshold: orchester_core::run::review::Probability::new(0.8).expect("threshold"),
    destinations,
  }
}

fn hosts(listed: &[&str]) -> ReviewerDestinations {
  ReviewerDestinations::Hosts(BTreeSet::from_iter(
    listed.iter().map(|host| ReviewerHost::new(host).expect("host")),
  ))
}

/// Writes the body of the earlier form (it names a connection) over the newest version of `reviewer`, as a cache from
/// before ORC-887 holds it.
fn make_old_form(team: &Team, reviewer: ResourceId) {
  let old = serde_json::json!({
    "kind": "language", "connection": {"kind": "connection", "name": "Team language", "role": "claude_code_cli"},
    "model": "m-1", "effort": null
  });
  let stored = serde_json::json!({"schema": <TeamReviewer as ResourceBody>::SCHEMA, "body": old}).to_string();
  let database = rusqlite::Connection::open(team.root.join(".orchester").join("orchester.db")).expect("database");
  let changed = database
    .execute(
      "UPDATE shared_version_bodies SET local_body = ?1 \
       WHERE version_id IN (SELECT id FROM resource_versions WHERE resource_id = ?2)",
      rusqlite::params![stored, reviewer.to_string()],
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

fn reviewers(team: &Team) -> (ResourceId, ResourceId) {
  let mut db = team.db();
  let language = team_resource(&mut db, "language-reviewer", language(), Vec::new());
  let decision = team_resource(
    &mut db,
    "decision-reviewer",
    decision(ReviewerDestinations::MembersComputers),
    Vec::new(),
  );
  (language, decision)
}

#[test]
fn team_fill_ins_pick_a_connection_of_a_language_reviewers_cli_kind() {
  let team = Team::new();
  let (reviewer, _) = reviewers(&team);
  let before = team.connection_changes();
  team
    .workspace
    .pick_team_reviewer_connection(&team.root, reviewer, team.claude)
    .expect("a Claude Code connection for a Claude Code reviewer");
  assert_eq!(
    team.db().connection_pick(reviewer).expect("read"),
    Some(pick(team.claude))
  );
  assert_eq!(team.connection_changes(), before + 1, "screens read again");

  let refused = team
    .workspace
    .pick_team_reviewer_connection(&team.root, reviewer, team.codex);
  assert!(
    matches!(refused, Err(FillInError::ConnectionDoesNotFit { .. })),
    "{refused:?}"
  );
  assert_eq!(
    team.db().connection_pick(reviewer).expect("read"),
    Some(pick(team.claude)),
    "the earlier pick stays"
  );
}

#[test]
fn team_fill_ins_a_reviewer_pick_names_the_teams_reviewer_and_the_kind_of_connection_it_runs_on() {
  let team = Team::new();
  let (language, decision) = reviewers(&team);
  let local = team
    .workspace
    .pick_team_reviewer_connection(&team.root, team.local_profile, team.claude);
  assert!(matches!(local, Err(FillInError::NotTeamResource { .. })), "{local:?}");
  let gone =
    team
      .workspace
      .pick_team_reviewer_connection(&team.root, language, orchester_core::id::ConnectionId::generate());
  assert!(matches!(gone, Err(FillInError::ConnectionNotFound(_))), "{gone:?}");
  let on_decision = team
    .workspace
    .pick_team_reviewer_connection(&team.root, decision, team.claude);
  assert!(
    matches!(on_decision, Err(FillInError::ReviewerIsDecision(_))),
    "{on_decision:?}"
  );
  let mut db = team.db();
  let server = decision_connection(&mut db, "local", "http://127.0.0.1:8000");
  let on_language = team
    .workspace
    .pick_team_decision_connection(&team.root, language, server, Answerer::Person);
  assert!(
    matches!(on_language, Err(FillInError::ReviewerIsLanguage(_))),
    "{on_language:?}"
  );
  assert_eq!(
    db.connection_pick(language).expect("read"),
    None,
    "a refusal keeps nothing"
  );
}

#[test]
fn team_fill_ins_a_decision_reviewer_runs_on_a_decision_connection_and_a_remote_one_is_a_persons_to_pick() {
  let team = Team::new();
  let mut db = team.db();
  let reviewer = team_resource(
    &mut db,
    "listing-reviewer",
    decision(hosts(&["api.typesafe.ai"])),
    Vec::new(),
  );
  let (local, hosted) = (
    decision_connection(&mut db, "local", "http://127.0.0.1:8000"),
    decision_connection(&mut db, "hosted", "https://api.typesafe.ai/v1"),
  );
  team
    .workspace
    .pick_team_decision_connection(&team.root, reviewer, local, Answerer::Automation)
    .expect("a decision server on this computer may be set up by automation");
  assert_eq!(db.decision_pick(reviewer).expect("read"), Some(local));

  let refused = team
    .workspace
    .pick_team_decision_connection(&team.root, reviewer, hosted, Answerer::Automation);
  assert!(matches!(refused, Err(FillInError::PersonOnly)), "{refused:?}");
  assert_eq!(db.decision_pick(reviewer).expect("read"), Some(local), "the pick stays");
  team
    .workspace
    .pick_team_decision_connection(&team.root, reviewer, hosted, Answerer::Person)
    .expect("a person sends requests to a hosted server the team lists");
  assert_eq!(db.decision_pick(reviewer).expect("read"), Some(hosted));

  let gone = team.workspace.pick_team_decision_connection(
    &team.root,
    reviewer,
    DecisionConnectionId::generate(),
    Answerer::Person,
  );
  assert!(
    matches!(gone, Err(FillInError::DecisionConnectionNotFound(_))),
    "{gone:?}"
  );
}

#[test]
fn team_fill_ins_a_decision_server_the_reviewer_does_not_list_is_refused_even_to_a_person() {
  let team = Team::new();
  let mut db = team.db();
  let members_only = team_resource(
    &mut db,
    "members-only",
    decision(ReviewerDestinations::MembersComputers),
    Vec::new(),
  );
  let other_host = team_resource(&mut db, "other-host", decision(hosts(&["other.example"])), Vec::new());
  let no_host = team_resource(&mut db, "no-host", decision(hosts(&[])), Vec::new());
  let (local, hosted) = (
    decision_connection(&mut db, "local", "http://127.0.0.1:8000"),
    decision_connection(&mut db, "hosted", "https://api.typesafe.ai/v1"),
  );
  for reviewer in [members_only, other_host, no_host] {
    let refused = team
      .workspace
      .pick_team_decision_connection(&team.root, reviewer, hosted, Answerer::Person);
    assert!(matches!(refused, Err(FillInError::ServerNotAllowed(_))), "{refused:?}");
    assert_eq!(
      db.decision_pick(reviewer).expect("read"),
      None,
      "a refusal keeps nothing"
    );
    team
      .workspace
      .pick_team_decision_connection(&team.root, reviewer, local, Answerer::Automation)
      .expect("a server on this computer is always allowed");
  }
}

#[test]
fn team_fill_ins_a_reviewer_that_names_a_connection_is_refused_as_not_in_the_teams_form() {
  let team = Team::new();
  let (language, decision) = reviewers(&team);
  let mut db = team.db();
  let server = decision_connection(&mut db, "local", "http://127.0.0.1:8000");
  make_old_form(&team, language);
  make_old_form(&team, decision);

  let refused = team
    .workspace
    .pick_team_reviewer_connection(&team.root, language, team.claude);
  assert!(
    matches!(refused, Err(FillInError::ReviewerNotTeamForm(_))),
    "{refused:?}"
  );
  let refused = team
    .workspace
    .pick_team_decision_connection(&team.root, decision, server, Answerer::Person);
  assert!(
    matches!(refused, Err(FillInError::ReviewerNotTeamForm(_))),
    "{refused:?}"
  );
  assert_eq!(db.connection_pick(language).expect("read"), None);
  assert_eq!(db.decision_pick(decision).expect("read"), None);
}
