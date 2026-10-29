//! `list_accounts` names the person's accounts without their emails or folders, with each one's latest plan limits
//! reading, and `set_connection_account` moves a connection to another account only while a person lets automation
//! widen; so do a profile saved on a connection of another account and auto mode's reviewer chosen on one.

use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::{
    AgentConnection, AgentProvider, ConnectionAccount,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, ReadingSource},
  },
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
  let refused = served.refused("set_approval_settings", &reviewer);
  assert_eq!(
    refused["code"], "person_only",
    "the reviewer on another account: {refused}"
  );
  served.let_automation_widen(true);
  served.ok("save_profile_draft", &moved);
  served.ok("set_approval_settings", &reviewer);
}

/// So the Orchestrator chat can say how much quota is left: each account's latest plan limits reading, from a read or a
/// turn, with when it was observed and how old it is; a window whose reset passed since says its use is unknown.
#[test]
fn domain_accounts_list_each_ones_latest_plan_limits_with_source_and_age() {
  let (mut served, seeded) = Served::seeded("connections");
  let client = add_account(&seeded, AccountCli::Codex, "Client");
  let lab = add_account(&seeded, AccountCli::ClaudeCode, "Lab");
  let now = Timestamp::now().unix_millis();
  let window = |minutes, percent, resets: i64| {
    LimitWindow::new(
      minutes,
      Some(percent),
      Some(Timestamp::from_unix_millis(now + resets)),
      LimitState::Normal,
      false,
    )
    .expect("a window")
  };
  let read = PlanLimits {
    windows: vec![window(300, 30.0, 3_600_000), window(10_080, 90.0, -60_000)],
    plan: Some("pro".to_owned()),
    credits: None,
    blocked: None,
  };
  let observed = Timestamp::from_unix_millis(now - 120_000);
  AppDb::open(&seeded.app_database)
    .expect("the app database")
    .save_plan_read((client.id(), AccountCli::Codex), observed, &read)
    .expect("the read is kept");
  served.services().runners.observe_plan_limits(
    LimitReading {
      provider: AgentProvider::ClaudeCodeCli,
      account: Some(lab.id()),
      at: Timestamp::now(),
      limits: PlanLimits {
        windows: vec![window(300, 12.0, 3_600_000)],
        plan: None,
        credits: None,
        blocked: None,
      },
    },
    ReadingSource::Turn,
  );

  let listed = served.ok("list_accounts", &json!({}));
  let plan = |name: &str| {
    listed["accounts"]
      .as_array()
      .and_then(|accounts| accounts.iter().find(|account| account["name"] == name))
      .unwrap_or_else(|| panic!("{name} is listed: {listed}"))["plan_limits"]
      .clone()
  };
  let client_plan = plan("Client");
  assert_eq!(client_plan["source"], "read", "{client_plan}");
  assert_eq!(client_plan["observed_at"], observed.unix_millis());
  assert!(
    client_plan["age_seconds"]
      .as_i64()
      .is_some_and(|age| (120..180).contains(&age)),
    "{client_plan}"
  );
  assert_eq!(client_plan["plan"], "pro");
  let windows = client_plan["windows"].as_array().expect("windows");
  let by = |minutes: u64| {
    windows
      .iter()
      .find(|window| window["minutes"] == minutes)
      .unwrap_or_else(|| panic!("the {minutes}-minute window: {client_plan}"))
  };
  assert_eq!(by(300)["used_percent"], 30.0);
  assert_eq!(by(300)["reset_since_reading"], false);
  assert!(
    by(10_080)["used_percent"].is_null(),
    "its reset passed: its use now is unknown"
  );
  assert_eq!(by(10_080)["reset_since_reading"], true);
  assert_eq!(plan("Lab")["source"], "turn");
  assert!(listed["accounts"].as_array().is_some_and(|accounts| {
    accounts
      .iter()
      .any(|account| account["default"] == true && account["cli"] == "claude_code")
  }));
}
