//! `list_accounts` names the person's accounts without their emails or folders, and `set_connection_account`
//! moves a connection to another account only while a person lets automation widen; so do a profile saved on a
//! connection of another account and auto mode's reviewer chosen on one.

use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::{AgentConnection, AgentProvider, ConnectionAccount},
  id::{AgentAccountId, ConnectionId},
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use serde_json::json;

use super::{Seeded, Served};

#[test]
fn domain_accounts_are_listed_and_chosen_for_a_connection_by_a_person_only() {
  let (mut served, seeded) = Served::seeded("connections");
  let client = AgentAccount::new(
    AgentAccountId::generate(),
    AccountCli::Codex,
    Name::new("Client").expect("a name"),
    AccountFolder::Created(seeded.app_database.with_file_name("codex-client")),
    Timestamp::now(),
  )
  .expect("an account");
  AppDb::open(&seeded.app_database)
    .expect("the app database")
    .add_agent_account(&client)
    .expect("added");

  let listed = served.ok("list_accounts", &json!({}));
  let accounts = listed["accounts"].as_array().expect("a list");
  assert_eq!(accounts.len(), 3, "{listed}");
  let named = accounts
    .iter()
    .find(|account| account["name"] == "Client")
    .expect("Client is listed");
  assert_eq!(named["cli"], "codex");
  assert_eq!(named["default"], false);
  assert_eq!(named["folder"], "created");
  assert!(named.get("email").is_none() && !listed.to_string().contains("codex-client"));

  let connections = served.ok("list_connections", &json!({ "project": "Kontur" }));
  let codex = connections["agents"]
    .as_array()
    .and_then(|agents| agents.iter().find(|agent| agent["provider"] == "codex_cli"))
    .expect("the seed's Codex connection")["name"]
    .clone();
  let choose = |account: &str| json!({ "project": "Kontur", "connection": codex, "account": account });
  served.let_automation_widen(false);
  let refused = served.refused("set_connection_account", &choose("Client"));
  assert_eq!(refused["code"], "person_only", "{refused}");
  served.let_automation_widen(true);
  let chosen = served.ok("set_connection_account", &choose("client"));
  assert_eq!(chosen["account"]["id"], client.id().to_string());
  let connections = served.ok("list_connections", &json!({ "project": "Kontur" }));
  assert!(
    connections["agents"]
      .as_array()
      .is_some_and(|agents| agents.iter().any(|agent| agent["account"] == client.id().to_string())),
    "{connections}"
  );
  let missing = served.refused("set_connection_account", &choose("Personal"));
  assert_eq!(missing["code"], "account_not_found", "{missing}");
  served.let_automation_widen(false);
  served.ok("set_connection_account", &choose("Client"));
  assert_eq!(
    served.refused("set_connection_account", &choose("default"))["code"],
    "person_only"
  );
}

/// Adds `cli`'s account `name` to the application database of `seeded`, in a folder made for it.
fn add_account(seeded: &Seeded, cli: AccountCli, name: &str) -> AgentAccount {
  let folder = seeded.app_database.with_file_name(format!("{}-{name}", cli.as_str()));
  std::fs::create_dir_all(&folder).expect("the account's folder");
  let account = AgentAccount::new(
    AgentAccountId::generate(),
    cli,
    Name::new(name).expect("a name"),
    AccountFolder::Created(folder),
    Timestamp::now(),
  )
  .expect("an account");
  AppDb::open(&seeded.app_database)
    .expect("the app database")
    .add_agent_account(&account)
    .expect("added");
  account
}

/// A connection called `name` of `provider`'s CLI on `account`, in the seeded project.
fn connect(seeded: &Seeded, (name, provider): (&str, AgentProvider), account: &AgentAccount) {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(name).expect("a name"),
    provider,
    std::env::current_exe().expect("a program"),
    Timestamp::now(),
  )
  .expect("a connection");
  connection
    .set_account(ConnectionAccount::of(account))
    .expect("an account of its CLI");
  ProjectDb::open(&seeded.repo)
    .expect("the project database")
    .create_connection(&connection.into_connection())
    .expect("saved");
}

#[test]
fn domain_profiles_and_the_reviewer_move_to_another_account_only_when_a_person_lets_automation_widen() {
  let (mut served, seeded) = Served::seeded("connections");
  let client = add_account(&seeded, AccountCli::Codex, "Client");
  let lab = add_account(&seeded, AccountCli::ClaudeCode, "Lab");
  connect(&seeded, ("Codex · Client", AgentProvider::CodexCli), &client);
  connect(&seeded, ("Claude · Lab", AgentProvider::ClaudeCodeCli), &lab);
  served.let_automation_widen(false);
  let profile = |role: &str, connection: &str| json!({ "project": "Kontur", "role": role, "connection": connection, "model": "gpt-5.5" });
  served.ok("save_profile_draft", &profile("Runner", "Codex · Kontur"));
  served.ok("publish_profile", &json!({ "project": "Kontur", "profile": "Runner" }));
  let refused = served.refused("save_profile_draft", &profile("Client runner", "Codex · Client"));
  assert_eq!(
    refused["code"], "person_only",
    "a new profile on another account: {refused}"
  );
  let mut moved = profile("Runner", "Codex · Client");
  moved["profile"] = json!("Runner");
  let refused = served.refused("save_profile_draft", &moved);
  assert_eq!(
    refused["code"], "person_only",
    "a profile moved to another account: {refused}"
  );
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("the setting is saved");
  let reviewer = json!({ "project": "Kontur", "reviewer": { "connection": "Claude · Lab" } });
  let refused = served.refused("set_auto_mode", &reviewer);
  assert_eq!(
    refused["code"], "person_only",
    "the reviewer on another account: {refused}"
  );
  served.let_automation_widen(true);
  served.ok("save_profile_draft", &moved);
  served.ok("set_auto_mode", &reviewer);
}
