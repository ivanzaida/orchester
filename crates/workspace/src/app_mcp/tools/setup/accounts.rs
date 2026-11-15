//! `list_accounts` and `check_account`: what agents can run on on this computer, for every project. The person's
//! agent CLI accounts by name, CLI and state, never the email or a folder (`docs/design/agent-accounts.md`,
//! "Security and privacy"), with each one's latest plan limits reading; their API providers and decision model
//! servers, never a key; and where each agent CLI's program is. Adding, renaming and removing one, signing in and
//! entering a key stay the person's, in the window. Which one a profile runs on is [`super::runs_on`].

use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount, ApiAccount, ApiSetup, same_sign_in},
  connection::{
    DecisionConnection,
    plan_limits::{LimitBlock, LimitReading, LimitState, PlanReadings},
  },
  id::{AgentAccountId, ApiAccountId, DecisionConnectionId},
  resource::ModelId,
  time::Timestamp,
};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::wire,
  runs_on::{Accounts, CliKind, Named},
};
use crate::{
  accounts::AccountsError,
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  connections::probe,
  links::Link,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct NoArguments {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct CheckAccount {
  /// A sign-in, an API provider or a decision model's server, by its id or its name (list_accounts); `default` is
  /// a CLI's Default and needs `cli`.
  pub account: String,
  /// The agent CLI whose account `account` names; needed for `default` and for a name two CLIs' accounts share.
  pub cli: Option<CliKind>,
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
    let providers: Vec<Value> = services
      .workspace
      .api_accounts()?
      .iter()
      .map(|account| {
        let mut listed = provider_json(account);
        // Whether the keychain has the key is all that is said of it; one that cannot be asked reads as missing.
        let here = services
          .workspace
          .api_account_has_key(services.vault.get(), account.id())
          .unwrap_or(false);
        listed["key_here"] = json!(here);
        listed
      })
      .collect();
    let servers: Vec<Value> = services.workspace.decision_servers()?.iter().map(server_json).collect();
    let programs = AccountCli::ALL
      .into_iter()
      .map(|cli| program_json(&services, cli))
      .collect::<Result<Vec<Value>, ToolError>>()?;
    Ok(json!({
      "accounts": listed,
      "api_providers": providers,
      "decision_servers": servers,
      "programs": programs,
    }))
  })
  .await
}

/// Where the program of `cli` is: the path the person set, and the copies on the search path. Nothing is run.
fn program_json(services: &Services, cli: AccountCli) -> Result<Value, ToolError> {
  let set = services.workspace.agent_program(cli)?;
  let found: Vec<String> = probe::detect(cli.program())
    .iter()
    .map(|copy| copy.display().to_string())
    .collect();
  Ok(json!({
    "cli": cli.as_str(),
    "set": set.map(|program| program.display().to_string()),
    "on_search_path": found,
  }))
}

/// What `check_account` checks.
enum Checked {
  SignIn(AgentAccountId),
  Api(ApiAccountId),
  Decision(DecisionConnectionId),
}

/// Checks one of the person's accounts again, as its page's *Check again* does: a sign-in's CLI is asked who is
/// signed in, a model API of Orchester's own agent and a decision model's server for their models.
pub async fn check_account(services: &Services, args: CheckAccount) -> Result<Value, ToolError> {
  let found = services.clone();
  let checked = blocking(move || {
    let servers = found.workspace.decision_servers()?;
    let asked = args.account.trim();
    let server = servers
      .iter()
      .find(|server| server.id().to_string() == asked || server.name().as_str().eq_ignore_ascii_case(asked));
    if let Some(server) = server {
      return Ok(Checked::Decision(server.id()));
    }
    Ok(
      match Accounts::load(&found)?.named(args.cli.map(CliKind::provider), &args.account)? {
        Named::SignIn(account) => Checked::SignIn(account.id()),
        Named::Api(account) => Checked::Api(account.id()),
      },
    )
  })
  .await?;
  let refused = |error: AccountsError| {
    let code = match error {
      AccountsError::NotTested => "not_tested",
      AccountsError::Keychain(_) => "key_missing",
      _ => "not_checked",
    };
    ToolError::refused(code, vec![error.message()])
  };
  match checked {
    Checked::SignIn(id) => {
      let services = services.clone();
      blocking(move || {
        let account = services.workspace.check_agent_account(id).map_err(refused)?;
        let accounts = services.workspace.agent_accounts()?;
        Ok(json!({ "account": account_json(&account, &accounts) }))
      })
      .await
    }
    Checked::Api(id) => {
      let account = services
        .workspace
        .check_api_account(services.vault.get(), id)
        .await
        .map_err(refused)?;
      Ok(json!({ "api_provider": provider_json(&account) }))
    }
    Checked::Decision(id) => {
      let server = services
        .workspace
        .check_decision_server(services.vault.get(), id)
        .await
        .map_err(|error| ToolError::refused("not_checked", vec![error.message()]))?;
      Ok(json!({ "decision_server": server_json(&server) }))
    }
  }
}

/// An API provider of the person's as `list_accounts` shows it: never its key.
pub(super) fn provider_json(account: &ApiAccount) -> Value {
  let (used_with, models): (&str, Vec<&str>) = match account.setup() {
    ApiSetup::Cli { cli, models, .. } => (cli.as_str(), models.iter().map(ModelId::as_str).collect()),
    ApiSetup::Native { models, .. } => (
      "orchester_agent",
      models.models().iter().map(|model| model.id().as_str()).collect(),
    ),
  };
  json!({
    "id": account.id().to_string(),
    "name": account.name().as_str(),
    "used_with": used_with,
    "host": account.setup().base_url().host(),
    "models": models,
    "status": account.last_check().map(|check| check.result.status().as_str()),
  })
}

/// A decision model server of the person's as `list_accounts` shows it: never its key.
fn server_json(server: &DecisionConnection) -> Value {
  json!({
    "id": server.id().to_string(),
    "name": server.name().as_str(),
    "host": server.base_url().host(),
    "takes_key": server.key().is_some(),
    "models": server.models().iter().map(ModelId::as_str).collect::<Vec<_>>(),
    "checked": server.last_check().is_some(),
  })
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

/// The account by name, CLI and state, with its link in Settings › Accounts: no email and no folder leave the
/// window.
pub(super) fn account_json(account: &AgentAccount, accounts: &[AgentAccount]) -> Value {
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
    "state": account.state().as_str(),
    "folder": folder,
    "same_sign_in_as": same_sign_in(account, accounts).map(|other| other.id().to_string()),
    "link": Link::Account(account.id()).path(),
  })
}
