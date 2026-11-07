//! `run_task` of a team task whose pipeline is not ready on this computer (`not_ready_here`): it is read from what this
//! computer has cached and filled in before the team server is asked for anything, so nothing is claimed, no work is
//! made and no attempt is counted. The answer lists the missing parts, typed, each with the fix; a pipeline that cannot
//! be decided is not assumed ready.

use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::ConnectionId,
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{ConnectionPick, TeamStartStage};
use serde_json::{Value, json};

use crate::{Checkout, ready, team_server::TeamServer, tools::server};

fn run() -> Value {
  json!({"project": "Kontur", "key": "KON-7"})
}

fn connection(checkout: &Checkout, provider: AgentProvider, label: &str) -> ConnectionId {
  let made = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(label).expect("name"),
    provider,
    checkout.root().join(format!("{label}.exe")),
    Timestamp::now(),
  )
  .expect("connection");
  checkout.db().create_connection(&made).expect("a connection");
  made.id()
}

/// The refusal of `run_task`, each message rendered as its key and values.
fn refusal(checkout: &Checkout) -> Value {
  checkout
    .call_rendering("run_task", &run(), |message| {
      let values: Vec<String> = message
        .args
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
      format!("{} {}", message.key, values.join(" "))
    })
    .expect_err("refused")
}

/// Nothing was claimed, made or counted on the server, and no start is kept here.
fn assert_nothing_started(server: &TeamServer, checkout: &Checkout) {
  let state = server.state();
  assert_eq!(state.works_made, 0, "the server made no work");
  assert!(state.claim.is_none(), "no claim was taken");
  drop(state);
  assert!(checkout.db().open_team_starts().expect("starts").is_empty());
}

#[test]
fn team_mcp_not_ready_here_a_missing_cli_connection_starts_nothing_and_lists_the_part() {
  let (server, _seen) = server();
  let config = server.state().config.clone();
  // The profile runs on Claude Code; this computer has only the checkout's Codex connection.
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    ready::team_ready(db, &config, AgentProvider::ClaudeCodeCli);
  });

  let error = refusal(&checkout);
  assert_eq!(error["error"]["code"], "not_ready_here");
  let parts = error["error"]["parts"].as_array().expect("the parts");
  assert_eq!(parts.len(), 1, "{error}");
  assert_eq!(parts[0]["kind"], "connection");
  assert_eq!(parts[0]["problem"], "none_of_kind");
  assert_eq!(parts[0]["cli"], "claude_code_cli");
  assert_eq!(parts[0]["name"], "builder");
  let reasons = error["error"]["reasons"].as_array().expect("the reasons");
  assert!(
    reasons[0]
      .as_str()
      .expect("a reason")
      .starts_with("notReadyHere.refused ")
  );
  assert!(
    reasons[1]
      .as_str()
      .expect("a reason")
      .starts_with("notReadyHere.connectionNoneOfKind ")
  );
  assert_nothing_started(&server, &checkout);

  // Adding the connection makes it ready: the start goes through the server.
  connection(&checkout, AgentProvider::ClaudeCodeCli, "claude");
  assert_eq!(checkout.refused("run_task", &run()), "runner_unavailable");
  assert_eq!(server.state().works_made, 1);
}

#[test]
fn team_mcp_not_ready_here_a_pick_among_several_connections_makes_it_ready() {
  let (server, _seen) = server();
  let config = server.state().config.clone();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    ready::team_ready(db, &config, AgentProvider::CodexCli);
  });
  let second = connection(&checkout, AgentProvider::CodexCli, "codex-work");

  let error = refusal(&checkout);
  assert_eq!(error["error"]["code"], "not_ready_here");
  assert_eq!(error["error"]["parts"][0]["problem"], "pick_needed");
  assert_nothing_started(&server, &checkout);

  checkout
    .db()
    .set_connection_pick(
      config.profile,
      ConnectionPick {
        connection: second,
        account: None,
      },
    )
    .expect("a pick");
  assert_eq!(checkout.refused("run_task", &run()), "runner_unavailable");
  let starts = checkout.db().open_team_starts().expect("starts");
  assert_eq!(starts.len(), 1);
  assert_eq!(starts[0].stage, TeamStartStage::InputsStored);
}

#[test]
fn team_mcp_not_ready_here_a_profile_this_computer_cannot_read_as_the_teams_is_undecided_not_ready() {
  let (server, _seen) = server();
  let config = server.state().config.clone();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    // The pipeline is here, and its profile was cached before the team configuration model: it names a connection.
    crate::pipelines::team_pipeline_running(db, config.pipeline, "delivery", config.profile);
    let local = crate::fixture::builder();
    let team = orchester_core::resource::TeamAgentProfile::new(
      local.body().role().clone(),
      String::new(),
      local.body().instructions().clone(),
      (
        AgentProvider::CodexCli,
        local.body().model().clone(),
        local.body().settings().clone(),
      ),
    );
    ready::cache_profile(db, config.profile, &team, json!(local.body()));
  });

  let error = refusal(&checkout);
  assert_eq!(error["error"]["code"], "not_ready_here");
  assert_eq!(error["error"]["parts"][0]["kind"], "undecided");
  assert_eq!(error["error"]["parts"][0]["why"], "not_team_form");
  assert_nothing_started(&server, &checkout);
}
