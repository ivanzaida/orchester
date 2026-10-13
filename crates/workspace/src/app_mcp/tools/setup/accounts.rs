//! `list_accounts` and `set_connection_account`: the person's agent CLI accounts by name, CLI and state, never the
//! email or a folder (`docs/design/agent-accounts.md`, "Security and privacy"), and choosing which account a
//! connection runs on. Adding, renaming and removing an account and signing it in stay the person's, in the window.

use orchester_core::{
  account::{AccountFolder, AgentAccount, same_sign_in},
  connection::invocation::Platform,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{connections::agent_named, widening};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  connections::data::set_account_in,
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
    let listed: Vec<Value> = accounts
      .iter()
      .map(|account| account_json(account, &accounts))
      .collect();
    Ok(json!({ "accounts": listed }))
  })
  .await
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

/// The account by name, CLI and state: no email and no folder leave the window.
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
  })
}
