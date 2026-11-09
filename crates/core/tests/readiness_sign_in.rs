//! A connection whose CLI is signed out is not ready (ORC-848 D11): a profile that would run on it says it needs
//! signing in, whether the connection is its pick or the only one of its kind.

use std::collections::BTreeMap;

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId},
  name::Name,
  resource::{ModelId, PromptText, ProviderSettings, TeamAgentProfile, TeamReviewer},
  sharing::{ConnectionProblem, Here, Missing, Needs, Readiness, readiness},
};
use serde_json::json;

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

fn body() -> TeamAgentProfile {
  TeamAgentProfile::new(
    Name::new("Builder").expect("valid name"),
    String::new(),
    PromptText::new("Build.").expect("valid prompt"),
    (
      CLAUDE,
      ModelId::new("claude-opus-5-5").expect("valid model"),
      ProviderSettings::new(BTreeMap::new()).expect("valid settings"),
    ),
  )
}

fn sign_in(profile: ResourceId, connection: ConnectionId) -> Missing {
  Missing::ConnectionSignIn {
    profile,
    cli: CLAUDE,
    connection,
  }
}

#[test]
fn the_only_connection_of_the_kind_being_signed_out_asks_to_sign_in() {
  let (id, only) = (ResourceId::generate(), ConnectionId::generate());
  let needs = Needs::profile(id, &body(), &[]).expect("needs");
  let here = Here::nothing().with_connection(only, CLAUDE);
  assert_eq!(readiness(&needs, &here), Readiness::READY);
  assert_eq!(
    readiness(&needs, &here.with_signed_out(only)).missing(),
    [sign_in(id, only)]
  );
}

#[test]
fn the_picked_connection_being_signed_out_asks_to_sign_in_even_if_another_is_fine() {
  let (id, picked, other) = (
    ResourceId::generate(),
    ConnectionId::generate(),
    ConnectionId::generate(),
  );
  let needs = Needs::profile(id, &body(), &[]).expect("needs");
  let here = Here::nothing()
    .with_connection(picked, CLAUDE)
    .with_connection(other, CLAUDE)
    .with_pick(id, picked)
    .with_signed_out(picked);
  assert_eq!(readiness(&needs, &here).missing(), [sign_in(id, picked)]);
  let elsewhere = Here::nothing()
    .with_connection(picked, CLAUDE)
    .with_connection(other, CLAUDE)
    .with_pick(id, picked)
    .with_signed_out(other);
  assert_eq!(
    readiness(&needs, &elsewhere),
    Readiness::READY,
    "it does not run on that one"
  );
}

#[test]
fn several_connections_and_no_pick_still_ask_for_a_pick_first() {
  let (id, first, second) = (
    ResourceId::generate(),
    ConnectionId::generate(),
    ConnectionId::generate(),
  );
  let needs = Needs::profile(id, &body(), &[]).expect("needs");
  let here = Here::nothing()
    .with_connection(first, CLAUDE)
    .with_connection(second, CLAUDE)
    .with_signed_out(first);
  assert_eq!(
    readiness(&needs, &here).missing(),
    [Missing::Connection {
      profile: id,
      cli: CLAUDE,
      problem: ConnectionProblem::PickNeeded
    }]
  );
}

#[test]
fn a_local_profile_and_a_reviewer_on_a_signed_out_connection_ask_to_sign_in_too() {
  let (id, own) = (ResourceId::generate(), ConnectionId::generate());
  let here = Here::nothing()
    .with_connection(own, CLAUDE)
    .with_pick(id, own)
    .with_signed_out(own);
  assert_eq!(
    readiness(&Needs::local_profile(id, CLAUDE, &[]), &here).missing(),
    [sign_in(id, own)]
  );
  let reviewer: TeamReviewer =
    serde_json::from_value(json!({"kind": "language", "cli": "claude_code_cli", "model": "m-1", "effort": null}))
      .expect("language");
  assert_eq!(
    readiness(&Needs::reviewer(id, &reviewer), &here).missing(),
    [sign_in(id, own)]
  );
}
