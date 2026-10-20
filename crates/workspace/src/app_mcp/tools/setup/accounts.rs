//! `list_accounts` and `set_connection_account`: the person's agent CLI accounts by name, CLI and state, never the
//! email or a folder (`docs/design/agent-accounts.md`, "Security and privacy"), with each one's latest plan limits
//! reading, and choosing which account a connection runs on. Adding, renaming and removing an account and signing it
//! in stay the person's, in the window.

use orchester_core::{
  account::{AccountFolder, AgentAccount, same_sign_in},
  connection::{
    invocation::Platform,
    plan_limits::{LimitBlock, LimitReading, LimitState, PlanReadings},
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{super::wire, connections::agent_named, widening};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  connections::data::set_account_in,
  links::Link,
  messages::Message,
  tasks::outside::Outside,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct NoArguments {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetConnectionAccount {
  /// The project's id, or its name.
  pub project: String,
  /// The agent connection's id, or its name.
  pub connection: String,
  /// The account's id, or its name among the accounts of the connection's CLI; `default` is the CLI's Default.
  pub account: String,
}

pub async fn list_accounts(services: &Services, _args: NoArguments) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let accounts = services.workspace.agent_accounts()?;
    let readings = latest_readings(&services)?;
    let now = Timestamp::now();
    let listed: Vec<Value> = accounts
      .iter()
      .map(|account| {
        let mut listed = account_json(account, &accounts);
        listed["plan_limits"] = plan_json(&readings, account, now);
        listed
      })
      .collect();
    Ok(json!({ "accounts": listed }))
  })
  .await
}

/// Each account's newest plan limits reading: this session's, from a turn or a read, or the last one kept where this
/// session has none newer.
fn latest_readings(services: &Services) -> Result<PlanReadings, ToolError> {
  let mut readings = services.runners.plan_limits().borrow().clone();
  let kept = services
    .workspace
    .last_plan_readings()
    .map_err(|error| ToolError::refused("plan_limits", vec![error.message()]))?;
  for account in kept.all() {
    if let Some(id) = account.account_id()
      && let Some(source) = kept.source_of(id)
    {
      // An older kept reading than this session's is not taken.
      readings.observe_from(account.clone(), source);
    }
  }
  Ok(readings)
}

/// The account's newest reading as its CLI gave it, with where it came from and how old it is at `now`; `null` when
/// none is known. Nothing is estimated: a window that reset since shows as reset with its last use unknown.
fn plan_json(readings: &PlanReadings, account: &AgentAccount, now: Timestamp) -> Value {
  let (Some(reading), Some(source)) = (readings.of_account(account.id()), readings.source_of(account.id())) else {
    return Value::Null;
  };
  json!({
    "source": source.as_str(),
    "observed_at": wire::time(reading.at),
    "age_seconds": (now.unix_millis() - reading.at.unix_millis()).max(0) / 1000,
    "state": state_name(reading.state(now)),
    "plan": reading.limits.plan,
    "windows": windows_json(reading, now),
    "blocked": reading.limits.blocked.map(|blocked| block_name(blocked.reason)),
  })
}

fn windows_json(reading: &LimitReading, now: Timestamp) -> Vec<Value> {
  reading
    .limits
    .windows
    .iter()
    .map(|window| {
      let reset = window.expired(now);
      json!({
        "minutes": window.minutes(),
        // Used before a reset that has passed since says nothing about now.
        "used_percent": if reset { None } else { window.percent() },
        "resets_at": window.resets().map(wire::time),
        "reset_since_reading": reset,
        "state": state_name(reading.window_state(window, now)),
      })
    })
    .collect()
}

const fn state_name(state: LimitState) -> &'static str {
  match state {
    LimitState::Normal => "normal",
    LimitState::Near => "near",
    LimitState::Reached => "reached",
  }
}

/// Why the CLI refuses new turns: a window used up, no credits left, the workspace's own limit, or another reason.
const fn block_name(reason: LimitBlock) -> &'static str {
  match reason {
    LimitBlock::Window => "window",
    LimitBlock::Credits => "credits",
    LimitBlock::Workspace => "workspace",
    LimitBlock::Other => "other",
  }
}

pub async fn set_connection_account(services: &Services, args: SetConnectionAccount) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let connection = agent_named(&ProjectDb::open(&open.repo_root)?, &args.connection)?;
    let accounts = services.workspace.agent_accounts()?;
    let account = accounts
      .iter()
      .filter(|account| account.cli().provider() == connection.provider())
      .find(|account| names(account, &args.account))
      .ok_or_else(|| ToolError::not_found(Missing::Account, &args.account))?;
    if connection.account().id(connection.provider()) != Some(account.id()) {
      widening::allowed(&services, vec![Message::new("appMcp.errors.accountChoice")])?;
    }
    set_account_in(&open.repo_root, connection.id(), account)
      .map_err(|error| ToolError::refused("project_data", vec![error.message()]))?;
    services.changes.record(open.project.id(), Outside::Connections);
    Ok(json!({
      "connection": connection.name().as_str(),
      "account": account_json(account, &accounts),
    }))
  })
  .await
}

/// Whether `named` names `account`: its id, or its name ignoring case (`default` for Default).
fn names(account: &AgentAccount, named: &str) -> bool {
  let lowered = named.trim().to_lowercase();
  account.id().to_string() == named.trim()
    || account
      .name()
      .is_some_and(|name| name.as_str().to_lowercase() == lowered)
    || (account.is_default() && lowered == "default")
}

/// The account by name, CLI and state, with its link in Settings › Accounts: no email and no folder leave the
/// window.
fn account_json(account: &AgentAccount, accounts: &[AgentAccount]) -> Value {
  let folder = match account.folder() {
    AccountFolder::Own => "cli_own",
    AccountFolder::Created(_) => "created",
    AccountFolder::Brought(_) => "brought",
  };
  json!({
    "id": account.id().to_string(),
    "cli": account.cli().as_str(),
    "name": account.name().map(|name| name.as_str().to_owned()),
    "default": account.is_default(),
    "state": account.state(Platform::current()).as_str(),
    "folder": folder,
    "same_sign_in_as": same_sign_in(account, accounts).map(|other| other.id().to_string()),
    "link": Link::Account(account.id()).path(),
  })
}
