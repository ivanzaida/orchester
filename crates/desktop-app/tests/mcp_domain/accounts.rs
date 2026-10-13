//! `list_accounts` names the person's accounts without their emails or folders, and `set_connection_account`
//! moves a connection to another account only while a person lets automation widen.

use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  id::AgentAccountId,
  name::Name,
  time::Timestamp,
};
use orchester_store::AppDb;
use serde_json::json;

use super::Served;

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
