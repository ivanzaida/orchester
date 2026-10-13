//! With window control on, an MCP client can open Edit connection's *Account* field, but choosing another account
//! there is a person's decision: a client's choice is not taken unless a person lets automation widen, as
//! `set_connection_account` is not. A person's choice is, and *Save* runs the connection on it.

use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::{AgentConnection, Connection, ConnectionAccount},
  id::AgentAccountId,
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use serde_json::json;

use super::{
  Served,
  kontur::{self, SeedRequest, scenarios},
};

fn codex(repo: &std::path::Path) -> Connection {
  ProjectDb::open(repo)
    .expect("the database opens")
    .connections()
    .expect("connections read")
    .into_iter()
    .find(|connection| connection.name().as_str() == "Codex · Kontur")
    .expect("the seed's Codex connection")
}

/// Index of the account picker's choice that says `label`.
fn option(served: &mut Served, label: &str) -> String {
  (0..8)
    .map(|index| format!("connection-account-option-{index}"))
    .find(|id| super::tools_window::text(served, id).contains(label))
    .unwrap_or_else(|| panic!("a choice {label}"))
}

#[test]
fn domain_window_input_cannot_choose_a_connections_account() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let seeded = kontur::seed(&SeedRequest {
    data_dir: folder.path().join("bin"),
    repo: folder.path().join("repo-connections"),
    scenario: scenarios::named("connections").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  })
  .expect("the scenario seeds");
  let account_folder = folder.path().join("accounts").join("codex-client");
  std::fs::create_dir_all(&account_folder).expect("the account's folder");
  let client = AgentAccount::new(
    AgentAccountId::generate(),
    AccountCli::Codex,
    Name::new("Client").expect("a name"),
    AccountFolder::Created(account_folder),
    Timestamp::now(),
  )
  .expect("an account");
  AppDb::open(&seeded.app_database)
    .expect("the app database")
    .add_agent_account(&client)
    .expect("added");
  let runner = folder.path().join("no-runner").join("orchester-runner");
  let mut served = Served::start_with(folder, &seeded.app_database, runner, true);
  let connection = codex(&seeded.repo);
  served.ok(
    "navigate",
    &json!({
      "route": "edit_connection", "project": seeded.project.to_string(), "connection": connection.id().to_string(),
    }),
  );
  served.until("the account field", |served| served.shows("connection-account"));
  served
    .click_by_client("connection-account")
    .expect("a client opens the menu");
  served.until("the menu", |served| served.shows("connection-account-option-0"));
  let choice = option(&mut served, "Client");
  served.click_by_client(&choice).expect("the click is delivered");
  for _ in 0..10 {
    served.pass();
  }
  assert_eq!(
    *codex(&seeded.repo).account(),
    ConnectionAccount::Default,
    "a client's choice is not taken"
  );
  served.click_by_person("connection-account");
  served.until("the menu", |served| served.shows("connection-account-option-0"));
  let choice = option(&mut served, "Client");
  served.click_by_person(&choice);
  served.until("Save", |served| served.shows("connection-edit-save"));
  for _ in 0..3 {
    served.pass();
  }
  served.click_by_person("connection-edit-save");
  served.until("the person's choice", |_| {
    codex(&seeded.repo).account().id(
      AgentConnection::try_from(codex(&seeded.repo))
        .expect("an agent")
        .provider(),
    ) == Some(client.id())
  });
}
