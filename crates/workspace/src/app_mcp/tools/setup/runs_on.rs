//! What an agent profile runs on, named by account (`docs/design/agent-accounts.md`, "Agents run on accounts"):
//! `set_profile_account` runs a profile on a sign-in or an API provider of the person's, `list_models` gives the
//! models one serves, and a profile's `runs_on` says which one it runs on here. No tool names the connection a
//! project keeps for an account: Orchester makes and keeps those.
//!
//! Moving a profile's work to another sign-in, or to an API provider's host, is the person's to decide: refused as
//! `person_only` unless a person turned on *Let automation change agent permissions*.

use std::path::Path;

use orchester_core::{
  account::{AgentAccount, ApiAccount, ApiSetup},
  connection::{AgentConnection, AgentProvider, ConnectionAccount, DecisionConnection},
  id::{AgentAccountId, ApiAccountId, ConnectionId, ResourceId, SecretId},
  resource::{ModelId, ResourceKind, TeamAgentProfile},
  store::ConnectionStore,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  accounts::{account_json, provider_json},
  profiles::profile_named,
  team_fill_ins, widening,
};
use crate::{
  agents::{
    data::ProjectAgents,
    form::{effort_options, listed_models},
  },
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  connections::data::agent_connections,
  messages::Message,
};

/// An agent CLI, where a tool has to say whose account a name is.
#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum CliKind {
  Codex,
  ClaudeCode,
}

impl CliKind {
  pub(in crate::app_mcp::tools) const fn provider(self) -> AgentProvider {
    match self {
      Self::Codex => AgentProvider::CodexCli,
      Self::ClaudeCode => AgentProvider::ClaudeCodeCli,
    }
  }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetProfileAccount {
  /// The project's id, or its name.
  pub project: String,
  /// The agent profile's id, or its role: a local project's own, or a team's.
  pub profile: String,
  /// A sign-in or an API provider of the profile's CLI, by its id or its name (list_accounts); `default` is the
  /// CLI's Default.
  pub account: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct AccountModels {
  /// The project's id, or its name: a sign-in's models are the ones its CLI listed for the project.
  pub project: String,
  /// A sign-in, an API provider or a decision model's server, by its id or its name (list_accounts); `default` is
  /// a CLI's Default and needs `cli`.
  pub account: String,
  /// The agent CLI whose account `account` names; needed for `default` and for a name two CLIs' accounts share.
  pub cli: Option<CliKind>,
}

/// One of the person's accounts, as a tool names it.
pub(in crate::app_mcp::tools) enum Named {
  SignIn(AgentAccount),
  Api(ApiAccount),
}

/// Everything a profile can run on, read once for a listing.
pub(in crate::app_mcp::tools) struct Accounts {
  pub sign_ins: Vec<AgentAccount>,
  pub api: Vec<ApiAccount>,
}

impl Accounts {
  pub(in crate::app_mcp::tools) fn load(services: &Services) -> Result<Self, ToolError> {
    Ok(Self {
      sign_ins: services.workspace.agent_accounts()?,
      api: services.workspace.api_accounts()?,
    })
  }

  /// The account `named` names: an id names one for certain; a name, ignoring case, one of `cli`'s accounts, or of
  /// any agent's when no CLI is given, where two that share it are refused. `default` is the Default of `cli`.
  pub(in crate::app_mcp::tools) fn named(&self, cli: Option<AgentProvider>, named: &str) -> Result<Named, ToolError> {
    let asked = named.trim();
    if let Some(found) = self.sign_ins.iter().find(|account| account.id().to_string() == asked) {
      return Ok(Named::SignIn(found.clone()));
    }
    if let Some(found) = self.api.iter().find(|account| account.id().to_string() == asked) {
      return Ok(Named::Api(found.clone()));
    }
    let lowered = asked.to_lowercase();
    let of_cli = |provider: AgentProvider| cli.is_none_or(|cli| cli == provider);
    let sign_ins = self
      .sign_ins
      .iter()
      .filter(|account| of_cli(account.cli().provider()))
      .filter(|account| {
        account
          .name()
          .is_some_and(|name| name.as_str().to_lowercase() == lowered)
          || (cli.is_some() && account.is_default() && lowered == "default")
      })
      .map(|account| Named::SignIn(account.clone()));
    let api = self
      .api
      .iter()
      .filter(|account| of_cli(account.agent()) && account.name().as_str().to_lowercase() == lowered)
      .map(|account| Named::Api(account.clone()));
    let mut matching = sign_ins.chain(api);
    let found = matching
      .next()
      .ok_or_else(|| ToolError::not_found(Missing::Account, named))?;
    if matching.next().is_some() {
      return Err(ToolError::InvalidArguments(format!(
        "more than one account is named {named:?}; name it by id, or give cli"
      )));
    }
    Ok(found)
  }

  /// What `connection` runs on as a profile's `runs_on` says it: the CLI and the sign-in or the API provider;
  /// `picked` is the sign-in a pick names beside the connection.
  pub(super) fn runs_on_json(&self, connection: &AgentConnection, picked: Option<AgentAccountId>) -> Value {
    let provider = connection.provider();
    if let Some(key) = connection.key() {
      let account = ApiAccountId::from_uuid(key.as_uuid());
      let listed = self.api.iter().find(|listed| listed.id() == account);
      return json!({
        "agent": provider,
        "account": Value::Null,
        // A key the project still holds itself names no provider of the person's.
        "api_provider": listed.map_or_else(|| json!({ "id": Value::Null, "name": connection.name().as_str() }), provider_json),
      });
    }
    let id = picked.or_else(|| connection.account().id(provider));
    let account = id.and_then(|id| self.sign_ins.iter().find(|account| account.id() == id));
    json!({
      "agent": provider,
      "account": account.map(|account| account_json(account, &self.sign_ins)),
      "api_provider": Value::Null,
    })
  }
}

/// The project's connection on the account `named` names (of `cli`, when given), made now when it has none: what
/// a profile or a reviewer saved on the account runs on.
pub(in crate::app_mcp::tools) fn account_connection(
  services: &Services,
  root: &Path,
  cli: Option<CliKind>,
  named: &str,
) -> Result<ConnectionId, ToolError> {
  let refused = |error: crate::accounts::RunsOnError| ToolError::refused("account", vec![error.message()]);
  match Accounts::load(services)?.named(cli.map(CliKind::provider), named)? {
    Named::SignIn(account) => services.workspace.connect_account(root, account.id()).map_err(refused),
    Named::Api(account) => services
      .workspace
      .connect_api_account(root, account.id())
      .map_err(refused),
  }
}

/// Runs a profile on one of the person's accounts on this computer, as choosing the account in the profile's *Runs
/// on* does: the project gets its connection on the account when it has none, and the profile's version stays.
pub async fn set_profile_account(services: &Services, args: SetProfileAccount) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let found = profile_and_cli(&services, &db, &open.repo_root, &args.profile)?;
    let cli = found
      .cli
      .ok_or_else(|| ToolError::refused("profile_cli", vec![Message::new("appMcp.errors.profileNoCli")]))?;
    let (profile, name) = (found.id, found.name.clone());
    let accounts = Accounts::load(&services)?;
    let now = runs_as(&db, &found)?;
    let refused = |error: crate::accounts::RunsOnError| ToolError::refused("account", vec![error.message()]);
    let account = match accounts.named(Some(cli), &args.account)? {
      Named::SignIn(account) => {
        // Work that moves to another sign-in of the person's is theirs to send there. A profile that ran on
        // nothing moves from nowhere, and a CLI's Default is never such a move.
        if now.is_some() && now != Some(On::SignIn(account.id())) && !account.is_default() {
          widening::allowed(&services, vec![Message::new("appMcp.errors.accountChoice")])?;
        }
        services
          .workspace
          .run_profile_on_account(&open.repo_root, profile, account.id())
          .map_err(refused)?;
        json!({ "account": account_json(&account, &accounts.sign_ins) })
      }
      Named::Api(account) => {
        // An API provider gets the project's prompts and code at its host, under its key.
        if now != Some(On::Api(account.id())) {
          let reason = Message::new("appMcp.widens.apiAccount")
            .with("profile", name.clone())
            .with("account", account.name().as_str().to_owned())
            .with("host", account.setup().base_url().host().to_owned());
          widening::allowed(&services, vec![reason])?;
        }
        services
          .workspace
          .run_profile_on_api_account(&open.repo_root, profile, account.id())
          .map_err(refused)?;
        json!({ "api_provider": provider_json(&account) })
      }
    };
    let mut answer = json!({ "profile": { "id": profile.to_string(), "name": name } });
    if let (Some(answer), Some(account)) = (answer.as_object_mut(), account.as_object()) {
      answer.extend(account.clone());
    }
    Ok(answer)
  })
  .await
}

/// The models an account serves, as a profile's *Runs on* offers them. Nothing is changed: a sign-in the project
/// has no connection on yet lists none, since its CLI has not been asked for the project.
pub async fn list_models(services: &Services, args: AccountModels) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let (_, db) = services.read_project(&args.project)?;
    if let Some(server) = decision_named(&db.decision_connections()?, &args.account) {
      return Ok(server);
    }
    let accounts = Accounts::load(&services)?;
    let named = accounts.named(args.cli.map(CliKind::provider), &args.account)?;
    let connections = agent_connections(db.connections()?);
    let here = connections.iter().find(|connection| match &named {
      Named::SignIn(account) => {
        connection.provider() == account.cli().provider()
          && connection.api_provider().is_none()
          && *connection.account() == ConnectionAccount::of(account)
      }
      Named::Api(account) => {
        connection.provider() == account.agent()
          && connection.key() == Some(SecretId::from_uuid(account.id().as_uuid()))
      }
    });
    let name = match &named {
      Named::SignIn(account) => account.name().map_or("default", |name| name.as_str()).to_owned(),
      Named::Api(account) => account.name().as_str().to_owned(),
    };
    let Some(connection) = here else {
      return Ok(not_connected(name, &named));
    };
    let models: Vec<Value> = listed_models(connection)
      .iter()
      .map(|model| json!({ "id": model.as_str(), "efforts": effort_options(connection, model) }))
      .collect();
    Ok(json!({
      "account": name,
      "agent": connection.provider(),
      "checked": connection.last_check().is_some(),
      // An API provider's models are typed by a person, not listed by the CLI; those of Orchester's own agent are
      // fetched from its endpoint or added by a person.
      "source": match (connection.api(), connection.api_provider()) {
        (Some(_), _) => json!("native"),
        (None, Some(_)) => json!("api_provider"),
        (None, None) => json!(connection.catalog().map(|catalog| catalog.source())),
      },
      "models": models,
    }))
  })
  .await
}

/// The models of an account the project has no connection on yet: an API provider's are the ones the person typed
/// for it; a sign-in's are not known until its CLI is asked, which choosing it for a profile does.
fn not_connected(name: String, named: &Named) -> Value {
  let (agent, source, models): (AgentProvider, Value, Vec<&str>) = match named {
    Named::SignIn(account) => (account.cli().provider(), Value::Null, Vec::new()),
    Named::Api(account) => match account.setup() {
      ApiSetup::Cli { models, .. } => (
        account.agent(),
        json!("api_provider"),
        models.iter().map(ModelId::as_str).collect(),
      ),
      ApiSetup::Native { models, .. } => (
        account.agent(),
        json!("native"),
        models.models().iter().map(|model| model.id().as_str()).collect(),
      ),
    },
  };
  json!({
    "account": name,
    "agent": agent,
    "checked": false,
    "source": source,
    "models": models.into_iter().map(|id| json!({ "id": id, "efforts": [] })).collect::<Vec<_>>(),
  })
}

/// The models of the decision model's server `named` names among `servers`, the project's, by id or name; `None`
/// when it names none.
fn decision_named(servers: &[DecisionConnection], named: &str) -> Option<Value> {
  let asked = named.trim();
  let by_id = servers.iter().find(|server| server.id().to_string() == asked);
  let found = by_id.or_else(|| {
    servers
      .iter()
      .find(|server| server.name().as_str().to_lowercase() == asked.to_lowercase())
  })?;
  let models: Vec<Value> = found
    .models()
    .iter()
    .map(|model| json!({ "id": model.as_str(), "efforts": [] }))
    .collect();
  Some(json!({
    "account": found.name().as_str(),
    "checked": found.last_check().is_some(),
    "source": "decision",
    "models": models,
  }))
}

/// A profile a tool names.
struct Profile {
  id: ResourceId,
  name: String,
  /// The agent CLI it runs with, when it names one.
  cli: Option<AgentProvider>,
  /// The connection a local profile's version names: what it runs on until a choice is made for it here.
  named: Option<ConnectionId>,
}

/// The profile `named` names in the project of `db`, a team's in a team project and the project's own otherwise.
fn profile_and_cli(services: &Services, db: &ProjectDb, repo_root: &Path, named: &str) -> Result<Profile, ToolError> {
  if db.sharing()?.is_some() {
    let profile = team_fill_ins::team_named(services, repo_root, ResourceKind::AgentProfile, named)?
      .ok_or_else(|| ToolError::not_found(Missing::Profile, named))?;
    let cli = profile
      .body::<TeamAgentProfile>()
      .and_then(Result::ok)
      .map(|team| team.cli());
    return Ok(Profile {
      id: profile.id,
      name: profile.name.as_str().to_owned(),
      cli,
      named: None,
    });
  }
  let agents = ProjectAgents::load_in(repo_root)?;
  let entry = profile_named(&agents, named)?;
  let on = db
    .connection(entry.profile.connection())?
    .and_then(|connection| AgentConnection::try_from(connection).ok())
    .map(|connection| connection.provider());
  let cli = on.or_else(|| AgentProvider::cli_of_model(entry.profile.model().as_str()));
  Ok(Profile {
    id: entry.id,
    name: entry.profile.role().as_str().to_owned(),
    cli,
    named: Some(entry.profile.connection()),
  })
}

/// What a profile runs on here.
#[derive(Clone, Copy, PartialEq, Eq)]
enum On {
  SignIn(AgentAccountId),
  Api(ApiAccountId),
}

/// What `profile` runs as on this computer now: what was chosen for it here, else what its version names, else,
/// for a team's profile, the only connection of its CLI the project has; `None` while it runs on nothing here.
fn runs_as(db: &ProjectDb, profile: &Profile) -> Result<Option<On>, ToolError> {
  let connection = match db.connection_pick(profile.id)? {
    Some(pick) => {
      if let Some(account) = pick.account {
        return Ok(Some(On::SignIn(account)));
      }
      Some(pick.connection)
    }
    None => match profile.named {
      Some(named) => Some(named),
      None => only_of(db, profile.cli)?,
    },
  };
  let Some(connection) = connection else {
    return Ok(None);
  };
  let connection = db
    .connection(connection)?
    .and_then(|connection| AgentConnection::try_from(connection).ok());
  Ok(connection.and_then(|connection| match connection.key() {
    Some(key) => Some(On::Api(ApiAccountId::from_uuid(key.as_uuid()))),
    None => connection.account().id(connection.provider()).map(On::SignIn),
  }))
}

/// The project's only connection of `cli`, which a team's profile of that CLI runs on without a choice.
fn only_of(db: &ProjectDb, cli: Option<AgentProvider>) -> Result<Option<ConnectionId>, ToolError> {
  let Some(cli) = cli else {
    return Ok(None);
  };
  let connections = agent_connections(db.connections()?);
  let mut of_kind = connections.iter().filter(|connection| connection.provider() == cli);
  let first = of_kind.next().map(|connection| connection.id());
  Ok(first.filter(|_| of_kind.next().is_none()))
}
