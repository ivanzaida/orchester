//! A team start binds its run to what this computer filled in, taking each team MCP server from the bundle's pinned
//! versions: the connection picked for the profile, the program path of each server that runs one, and the secrets by
//! name. A bundle that pins what this computer lacks settles the start failed with the missing parts, and starts
//! nothing.

use std::{collections::HashMap, fs, path::PathBuf};

use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::ConnectionId,
  name::Name,
  resource::TeamMcpServer,
  run::inputs::team_mcp_id,
  secret::{SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  sharing::{ConnectionProblem, Missing, ProgramProblem},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{ConnectionPick, TeamStartStage};
use orchester_workspace::{
  readiness::McpPrograms,
  team_work::{StartFailure, StartOutcome},
};
use parking_lot::Mutex;
use serde_json::json;

use crate::{
  Checkout, StoreRunner,
  fixture::Config,
  team_server::{MEMBER, TeamServer},
};

const TOKEN: &str = "program-token-value-31c7";
const KEY: &str = "remote-key-value-8d20";

#[derive(Default)]
struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::NotFound)
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

/// The team's two MCP servers: one runs a program that takes `API_TOKEN`, one is remote and sends `API_KEY` after a
/// prefix.
fn servers() -> Vec<TeamMcpServer> {
  vec![
    serde_json::from_value(json!({
      "name": "docs",
      "launch": {"kind": "program", "program": "docs-mcp", "args": ["serve"], "variables": ["API_TOKEN"]}
    }))
    .expect("a program server"),
    serde_json::from_value(json!({
      "name": "issues",
      "launch": {"kind": "remote", "url": "https://mcp.example.org/issues",
                 "secret_headers": [{"header": "Authorization", "secret": "API_KEY", "prefix": "Bearer "}]}
    }))
    .expect("a remote server"),
  ]
}

fn secret(name: &str) -> SecretName {
  SecretName::new(name).expect("secret name")
}

/// Fills in, as the person did: a second Codex connection picked for the profile, both servers switched on, the program
/// a file, and a value for both secrets. The picked connection and the program's path.
fn fill_in(checkout: &Checkout, config: &Config) -> (ConnectionId, PathBuf) {
  let vault = MemoryVault::default();
  let [docs, issues] = config.mcp_servers[..] else {
    panic!("two servers");
  };
  let second = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex work").expect("name"),
    AgentProvider::CodexCli,
    checkout.root().join("codex-work.exe"),
    Timestamp::now(),
  )
  .expect("connection");
  let program = checkout.root().join("docs-mcp");
  fs::write(&program, "#!/bin/sh\n").expect("a program");
  let mut db = checkout.db();
  db.create_connection(&second).expect("a second connection");
  let pick = ConnectionPick {
    connection: second.id(),
    account: None,
  };
  db.set_connection_pick(config.profile, pick).expect("a pick");
  db.set_team_mcp_on_here(docs, true).expect("on");
  db.set_team_mcp_on_here(issues, true).expect("on");
  for (name, text) in [("API_TOKEN", TOKEN), ("API_KEY", KEY)] {
    let value = SecretValue::new(text.to_owned()).expect("value");
    db.set_team_secret_value(&vault, &secret(name), &value)
      .expect("a value");
  }
  (second.id(), program)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_fill_ins_a_ready_start_binds_the_picked_connection_programs_and_secrets() {
  let config = Config::with_mcp_servers("Append one line.", servers());
  let [docs, issues] = config.mcp_servers[..] else {
    panic!("two servers");
  };
  let server = TeamServer::start(config.clone());
  let checkout = Checkout::linked(&server);
  let (second, program) = fill_in(&checkout, &config);

  let claim = checkout.claim(&server, MEMBER).await;
  let mut request = checkout.start_request(&server, claim);
  request.programs = McpPrograms::from_iter([(docs, program.clone())]);
  let operation = request.operation;
  let runner = StoreRunner::new(checkout.root(), &[]);
  let (work, _generation) = checkout.work(&server, MEMBER, runner.clone());
  let outcome = work.start(request).await.expect("started");
  let StartOutcome::Started(run) = outcome else {
    panic!("not started: {outcome:?}");
  };

  let db = checkout.db();
  let inputs = db.run_inputs(run).expect("read").expect("the run's inputs");
  let bound = &inputs.bindings().profiles[0];
  assert_eq!(bound.connection, second, "the picked connection, not the first");
  assert_eq!(bound.mcp_servers, [team_mcp_id(docs), team_mcp_id(issues)]);
  let team = &inputs.bindings().team_mcp;
  assert_eq!(team.len(), 2);
  let program_of = |server| {
    team
      .iter()
      .find(|bound| bound.server == server)
      .expect("bound")
      .program
      .clone()
  };
  assert_eq!(program_of(docs), Some(program), "the program path set here");
  assert_eq!(program_of(issues), None, "a remote server runs no program here");
  let (pinned, path) = inputs.team_mcp_server(team_mcp_id(docs)).expect("the program server");
  assert_eq!(
    pinned.name().as_str(),
    "docs",
    "the server is the bundle's pinned version"
  );
  assert!(path.is_some());

  // No value is kept with the start or its inputs: they are read by name when an agent starts.
  let kept = db.team_start(operation).expect("read").expect("the start").request;
  let stored = serde_json::to_string(&inputs.bindings()).expect("json");
  for text in [kept, stored] {
    assert!(!text.contains(TOKEN) && !text.contains(KEY), "{text}");
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_fill_ins_a_bundle_that_pins_what_is_missing_here_fails_with_the_parts() {
  let config = Config::with_mcp_servers("Append one line.", servers()).running(AgentProvider::CodexCli);
  let [docs, issues] = config.mcp_servers[..] else {
    panic!("two servers");
  };
  let server = TeamServer::start(config.clone());
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  // The program server is on, but its program is gone; the remote one nobody decided on; no secret has a value.
  let gone = checkout.root().join("moved-away");
  checkout.db().set_team_mcp_on_here(docs, true).expect("on");
  let mut request = checkout.start_request(&server, claim);
  request.programs = McpPrograms::from_iter([(docs, gone.clone())]);
  let runner = StoreRunner::new(checkout.root(), &[]);
  let (work, _generation) = checkout.work(&server, MEMBER, runner.clone());

  let outcome = work.start(request.clone()).await.expect("carried on");
  let StartOutcome::Failed(StartFailure::NotReadyHere { pipeline, missing }) = outcome else {
    panic!("not failed as not ready: {outcome:?}");
  };
  assert_eq!(pipeline, "Delivery");
  let parts: Vec<&Missing> = missing.iter().map(|part| &part.missing).collect();
  assert_eq!(
    parts,
    [
      &Missing::McpOff { server: issues },
      &Missing::McpProgram {
        server: docs,
        problem: ProgramProblem::FileGone(gone)
      },
      &Missing::Secret {
        name: secret("API_TOKEN")
      },
      &Missing::Secret {
        name: secret("API_KEY")
      },
    ]
  );
  let names: Vec<&str> = missing.iter().map(|part| part.name.as_str()).collect();
  assert_eq!(names, ["issues", "docs", "API_TOKEN", "API_KEY"]);

  assert_eq!(runner.asked(), 0, "the runner was never asked");
  let kept = checkout
    .db()
    .team_start(request.operation)
    .expect("read")
    .expect("the start");
  assert_eq!(kept.stage, TeamStartStage::Failed);
  assert!(
    checkout
      .db()
      .accepted_team_inputs(request.operation)
      .expect("read")
      .is_none(),
    "no inputs were kept"
  );
  let again = work.start(request).await.expect("carried on");
  assert!(
    matches!(again, StartOutcome::Failed(StartFailure::NotReadyHere { .. })),
    "the failure is kept: {again:?}"
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_fill_ins_several_connections_without_a_pick_are_not_ready() {
  let config = Config::new("Append one line.");
  let server = TeamServer::start(config.clone());
  let checkout = Checkout::linked(&server);
  let second = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex work").expect("name"),
    AgentProvider::CodexCli,
    checkout.root().join("codex-work.exe"),
    Timestamp::now(),
  )
  .expect("connection");
  checkout.db().create_connection(&second).expect("a second connection");
  let claim = checkout.claim(&server, MEMBER).await;
  let request = checkout.start_request(&server, claim);
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));

  let outcome = work.start(request).await.expect("carried on");
  let StartOutcome::Failed(StartFailure::NotReadyHere { missing, .. }) = outcome else {
    panic!("not failed as not ready: {outcome:?}");
  };
  assert!(
    matches!(
      missing[0].missing,
      Missing::Connection {
        cli: AgentProvider::CodexCli,
        problem: ConnectionProblem::PickNeeded,
        ..
      }
    ),
    "{missing:?}"
  );
}
