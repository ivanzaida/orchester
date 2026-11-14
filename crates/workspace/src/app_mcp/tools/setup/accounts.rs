//! `list_accounts`, `set_profile_account` and `set_connection_account`: the person's agent CLI accounts by name, CLI
//! and state, never the email or a folder (`docs/design/agent-accounts.md`, "Security and privacy"), with each
//! one's latest plan limits reading; their API providers and decision model servers, never a key; and choosing
//! which account a profile, or a connection, runs on. Adding, renaming and removing an account and signing it
//! in stay the person's, in the window.

use orchester_core::{
  account::{AccountFolder, AgentAccount, ApiAccount, ApiSetup, same_sign_in},
  connection::{
    AgentConnection, AgentProvider, DecisionConnection,
    plan_limits::{LimitBlock, LimitReading, LimitState, PlanReadings},
  },
  id::{AgentAccountId, ResourceId},
  resource::{ModelId, ResourceKind, TeamAgentProfile},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{super::wire, connections::agent_named, profiles::profile_named, team_fill_ins, widening};
use crate::{
  agents::data::ProjectAgents,
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

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetProfileAccount {
  /// The project's id, or its name.
  pub project: String,
  /// The agent profile's id, or its role: a local project's own, or a team's.
  pub profile: String,
  /// The account's id, or its name among the accounts of the profile's CLI; `default` is the CLI's Default.
  pub account: String,
}

/// Runs a profile on one of the person's accounts on this computer, as choosing the account in the profile's *Runs
/// on* does: the project gets its connection on the account when it has none, and the profile's version stays.
pub async fn set_profile_account(services: &Services, args: SetProfileAccount) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let (profile, name, cli) = profile_and_cli(&services, &db, &open.repo_root, &args.profile)?;
    let cli = cli.ok_or_else(|| ToolError::refused("profile_cli", vec![Message::new("appMcp.errors.profileNoCli")]))?;
    let account = account_for(&services, cli, &args.account)?;
    // Work that moves to another account of the person's is theirs to send there. A profile that ran on nothing
    // moves from nowhere, and a CLI's Default is never such a move.
    let now = runs_as(&db, profile)?;
    if now.is_some() && now != Some(account.id()) && !account.is_default() {
      widening::allowed(&services, vec![Message::new("appMcp.errors.accountChoice")])?;
    }
    services
      .workspace
      .run_profile_on_account(&open.repo_root, profile, account.id())
      .map_err(|error| ToolError::refused("account", vec![error.message()]))?;
    let accounts = services.workspace.agent_accounts()?;
    Ok(json!({
      "profile": { "id": profile.to_string(), "name": name },
      "account": account_json(&account, &accounts),
    }))
  })
  .await
}

/// The profile `named` names in the project of `db`, a team's in a team project and the project's own otherwise:
/// its id, its name, and the agent CLI it runs with, when it names one.
fn profile_and_cli(
  services: &Services,
  db: &ProjectDb,
  repo_root: &std::path::Path,
  named: &str,
) -> Result<(ResourceId, String, Option<AgentProvider>), ToolError> {
  if db.sharing()?.is_some() {
    let profile = team_fill_ins::team_named(services, repo_root, ResourceKind::AgentProfile, named)?
      .ok_or_else(|| ToolError::not_found(Missing::Profile, named))?;
    let cli = profile
      .body::<TeamAgentProfile>()
      .and_then(Result::ok)
      .map(|team| team.cli());
    return Ok((profile.id, profile.name.as_str().to_owned(), cli));
  }
  let agents = ProjectAgents::load_in(repo_root)?;
  let entry = profile_named(&agents, named)?;
  let on = db
    .connection(entry.profile.connection())?
    .and_then(|connection| AgentConnection::try_from(connection).ok())
    .map(|connection| connection.provider());
  let cli = on.or_else(|| AgentProvider::cli_of_model(entry.profile.model().as_str()));
  Ok((entry.id, entry.profile.role().as_str().to_owned(), cli))
}

/// The account the profile runs as on this computer now: its pick's, else the account of the connection it is
/// picked on; `None` while nothing is chosen for it here.
fn runs_as(db: &ProjectDb, profile: ResourceId) -> Result<Option<AgentAccountId>, ToolError> {
  let Some(pick) = db.connection_pick(profile)? else {
    return Ok(None);
  };
  let of_connection = || -> Result<Option<AgentAccountId>, ToolError> {
    Ok(
      db.connection(pick.connection)?
        .and_then(|connection| AgentConnection::try_from(connection).ok())
        .and_then(|connection| connection.account().id(connection.provider())),
    )
  };
  match pick.account {
    Some(account) => Ok(Some(account)),
    None => of_connection(),
  }
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
    let providers: Vec<Value> = services.workspace.api_accounts()?.iter().map(provider_json).collect();
    let servers: Vec<Value> = services.workspace.decision_servers()?.iter().map(server_json).collect();
    Ok(json!({ "accounts": listed, "api_providers": providers, "decision_servers": servers }))
  })
  .await
}

/// An API provider of the person's as `list_accounts` shows it: never its key.
fn provider_json(account: &ApiAccount) -> Value {
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

pub async fn set_connection_account(services: &Services, args: SetConnectionAccount) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let connection = agent_named(&ProjectDb::open(&open.repo_root)?, &args.connection)?;
    let accounts = services.workspace.agent_accounts()?;
    let account = account_of(&accounts, connection.provider(), &args.account)?;
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

/// The account of `accounts` that `named` names among the accounts of the CLI of `provider`.
fn account_of<'a>(
  accounts: &'a [AgentAccount],
  provider: AgentProvider,
  named: &str,
) -> Result<&'a AgentAccount, ToolError> {
  accounts
    .iter()
    .filter(|account| account.cli().provider() == provider)
    .find(|account| names(account, named))
    .ok_or_else(|| ToolError::not_found(Missing::Account, named))
}

/// The account `named` names among the listed accounts of the CLI of `provider`.
pub(super) fn account_for(
  services: &Services,
  provider: AgentProvider,
  named: &str,
) -> Result<AgentAccount, ToolError> {
  account_of(&services.workspace.agent_accounts()?, provider, named).cloned()
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
    "state": account.state().as_str(),
    "folder": folder,
    "same_sign_in_as": same_sign_in(account, accounts).map(|other| other.id().to_string()),
    "link": Link::Account(account.id()).path(),
  })
}
