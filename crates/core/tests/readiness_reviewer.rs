//! A team reviewer's connection is a readiness need on this computer (`docs/design/team-configuration.md`,
//! "Readiness"): an agent connection of its CLI kind for a language reviewer, a decision connection for a decision one.

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, DecisionConnectionId, ResourceId},
  resource::TeamReviewer,
  sharing::{ConnectionProblem, Here, Missing, Needs, Readiness, readiness},
};
use serde_json::json;

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;
const CODEX: AgentProvider = AgentProvider::CodexCli;

fn language(cli: &str) -> TeamReviewer {
  serde_json::from_value(json!({"kind": "language", "cli": cli, "model": "m-1", "effort": null})).expect("language")
}

fn decision() -> TeamReviewer {
  serde_json::from_value(json!({
    "kind": "decision", "model": "d-1", "lowest_threshold": 0.8, "destinations": {"kind": "members_computers"}
  }))
  .expect("decision")
}

#[test]
fn a_language_reviewer_needs_a_connection_of_its_cli_kind_like_a_profile() {
  let id = ResourceId::generate();
  let needs = Needs::reviewer(id, &language("claude_code_cli"));
  // A connection of another kind does not count.
  let here = Here::nothing().with_connection(ConnectionId::generate(), CODEX);
  assert_eq!(
    readiness(&needs, &here).missing(),
    [Missing::Connection {
      profile: id,
      cli: CLAUDE,
      problem: ConnectionProblem::NoneOfKind
    }]
  );
  let (first, second) = (ConnectionId::generate(), ConnectionId::generate());
  let here = Here::nothing()
    .with_connection(first, CLAUDE)
    .with_connection(second, CLAUDE);
  assert_eq!(
    readiness(&needs, &here).missing(),
    [Missing::Connection {
      profile: id,
      cli: CLAUDE,
      problem: ConnectionProblem::PickNeeded
    }]
  );
  assert_eq!(readiness(&needs, &here.with_pick(id, second)), Readiness::READY);
  let only = Here::nothing().with_connection(first, CLAUDE);
  assert_eq!(readiness(&needs, &only), Readiness::READY);
}

#[test]
fn a_decision_reviewer_needs_a_decision_connection_and_an_agent_connection_does_not_count() {
  let id = ResourceId::generate();
  let needs = Needs::reviewer(id, &decision());
  let agent_only = Here::nothing().with_connection(ConnectionId::generate(), CLAUDE);
  assert_eq!(
    readiness(&needs, &agent_only).missing(),
    [Missing::DecisionConnection {
      reviewer: id,
      problem: ConnectionProblem::NoneOfKind
    }]
  );
  let (first, second) = (DecisionConnectionId::generate(), DecisionConnectionId::generate());
  let one = Here::nothing().with_decision_connection(first);
  assert_eq!(readiness(&needs, &one), Readiness::READY);
  let two = one.with_decision_connection(second);
  assert_eq!(
    readiness(&needs, &two).missing(),
    [Missing::DecisionConnection {
      reviewer: id,
      problem: ConnectionProblem::PickNeeded
    }]
  );
  assert_eq!(
    readiness(&needs, &two.clone().with_decision_pick(id, second)),
    Readiness::READY
  );
  let gone = two.with_decision_pick(id, DecisionConnectionId::generate());
  assert_eq!(
    readiness(&needs, &gone).missing(),
    [Missing::DecisionConnection {
      reviewer: id,
      problem: ConnectionProblem::PickNeeded
    }],
    "a pick of a connection that is not here counts as no pick"
  );
}

#[test]
fn a_reviewer_pick_is_its_own_and_missing_connections_are_listed_first() {
  let (reviewer, other) = (ResourceId::generate(), ResourceId::generate());
  let (first, second) = (ConnectionId::generate(), ConnectionId::generate());
  let here = Here::nothing()
    .with_connection(first, CLAUDE)
    .with_connection(second, CLAUDE)
    .with_pick(reviewer, first);
  assert_eq!(
    readiness(&Needs::reviewer(reviewer, &language("claude_code_cli")), &here),
    Readiness::READY
  );
  assert_eq!(
    readiness(&Needs::reviewer(other, &language("claude_code_cli")), &here).missing(),
    [Missing::Connection {
      profile: other,
      cli: CLAUDE,
      problem: ConnectionProblem::PickNeeded
    }]
  );
}
