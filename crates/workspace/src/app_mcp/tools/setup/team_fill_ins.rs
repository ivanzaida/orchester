//! What a computer fills in for a team project, over MCP (`docs/design/team-configuration.md`, "What a computer fills
//! in"): `pick_profile_connection` chooses the connection (and account) a team profile runs on here, and the team MCP
//! servers `set_mcp_server_enabled` and `update_mcp_server` name are the team's resources, not the project's own
//! servers. Turning a team MCP server on and setting its program path let agents run more here, which the person
//! decided is theirs alone, in the window (decision 12): over MCP they answer `person_only` always, which the switch
//! *Let automation change agent permissions* does not lift, before the write of [`crate::team_fill_ins`] is reached.
//! Turning one off is taken. No tool takes or returns a secret value.

use std::path::Path;

use orchester_core::{
  resource::{Resource, ResourceKind},
  run::Answerer,
  store::ResourceStore,
};
use orchester_store::{ConnectionPick, ProjectDb, ResourceSharing};
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{Spec, parse},
  accounts,
  connections::agent_named,
  widening,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
  team_fill_ins::FillInError,
};

pub(super) const SPECS: &[Spec] = &[Spec {
  name: "pick_profile_connection",
  description: "Chooses the connection a team profile runs on on this computer, as the profile's Connection field \
                does in a team project: `profile` is a team profile by id or role, `connection` an agent connection \
                of the project by id or name, of the CLI kind the profile names (a profile that names none takes any \
                agent connection). The pick is kept on this computer only and a connection added later changes \
                nothing. `account` (an account of the connection's CLI by id or name, list_accounts; `default` is the \
                CLI's Default) is for a connection that needs one; choosing another account than the one the \
                profile runs on now (its earlier pick's, else the connection's own) is refused as person_only unless \
                a person turned on Let automation change agent permissions. Refused as team_project_required in a \
                project that is not shared with a team and as profile_not_found for a profile that is not the \
                team's. Returns the pick.",
  schema: schema_for_type::<PickProfileConnection>,
  read_only: false,
}];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct PickProfileConnection {
  /// The project's id, or its name.
  pub project: String,
  /// The team profile's id, or its role.
  pub profile: String,
  /// The agent connection's id, or its name.
  pub connection: String,
  /// The account of the connection's CLI, by id or name; `default` is the CLI's Default.
  pub account: Option<String>,
}

/// Calls tool `name` of [`SPECS`] with `arguments`.
pub(super) async fn call(services: &Services, name: &str, arguments: Value) -> Result<Value, ToolError> {
  match name {
    "pick_profile_connection" => pick_profile_connection(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn pick_profile_connection(services: &Services, args: PickProfileConnection) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    require_team_project(&db)?;
    let profile = team_resource_named(&db, ResourceKind::AgentProfile, &args.profile)?
      .ok_or_else(|| ToolError::not_found(Missing::Profile, &args.profile))?;
    let connection = agent_named(&db, &args.connection)?;
    let account = match &args.account {
      Some(named) => Some(accounts::account_for(&services, connection.provider(), named)?),
      None => None,
    };
    if let Some(account) = &account {
      // The account the profile runs on now: its earlier pick's, else the connection's own.
      let effective = db
        .connection_pick(profile.id())?
        .and_then(|earlier| earlier.account)
        .or_else(|| connection.account().id(connection.provider()));
      if effective != Some(account.id()) {
        widening::allowed(&services, vec![Message::new("appMcp.errors.accountChoice")])?;
      }
    }
    let pick = ConnectionPick {
      connection: connection.id(),
      account: account.as_ref().map(|account| account.id()),
    };
    services
      .workspace
      .pick_team_connection(&open.repo_root, profile.id(), pick)
      .map_err(refusal)?;
    Ok(json!({
      "profile": { "id": profile.id().to_string(), "name": profile.name().as_str() },
      "connection": { "id": connection.id().to_string(), "name": connection.name().as_str() },
      "account": account.map(|account| account.id().to_string()),
    }))
  })
  .await
}

/// Turns the team MCP server `server` off here, as `set_mcp_server_enabled` does; on is a person's alone, so it is
/// refused here, with nothing changed and without reaching the write that would take it from a person.
pub(super) fn switch_server(
  services: &Services,
  repo_root: &Path,
  server: &Resource,
  enabled: bool,
) -> Result<Value, ToolError> {
  if enabled {
    return Err(person_only("appMcp.errors.teamMcpOnPersonOnly"));
  }
  services
    .workspace
    .switch_team_mcp_on_here(repo_root, server.id(), false, Answerer::Automation)
    .map_err(refusal)?;
  Ok(
    json!({ "server": { "id": server.id().to_string(), "name": server.name().as_str(), "team": true, "enabled": false } }),
  )
}

/// `update_mcp_server` giving a team MCP server a `program`: the path of the file agents run on this computer is a
/// person's alone, so it is refused whatever else the call changes.
pub(super) fn program_person_only() -> ToolError {
  person_only("appMcp.errors.teamMcpProgramPersonOnly")
}

/// The team's resource of `kind` that `named` names, by its id or its name; `None` in a project that is not shared
/// with a team, and for a name that is none of the team's.
pub(super) fn team_resource_named(
  db: &ProjectDb,
  kind: ResourceKind,
  named: &str,
) -> Result<Option<Resource>, ToolError> {
  if db.sharing()?.is_none() {
    return Ok(None);
  }
  let mut by_name = None;
  for resource in db.resources(kind)? {
    if !matches!(db.resource_sharing(resource.id())?, Some(ResourceSharing::Shared(_))) {
      continue;
    }
    // An id names one resource for certain; a name could also read as another's id.
    if resource.id().to_string() == named {
      return Ok(Some(resource));
    }
    if by_name.is_none() && resource.name().as_str() == named {
      by_name = Some(resource);
    }
  }
  Ok(by_name)
}

fn require_team_project(db: &ProjectDb) -> Result<(), ToolError> {
  if db.sharing()?.is_some() {
    Ok(())
  } else {
    Err(refusal(FillInError::NotTeamProject))
  }
}

fn person_only(reason: &'static str) -> ToolError {
  ToolError::refused(
    "person_only",
    vec![Message::new("appMcp.errors.personOnly"), Message::new(reason)],
  )
}

/// A fill-in the workspace refused, with the code a client matches on and the words the window shows.
fn refusal(error: FillInError) -> ToolError {
  let code = match error {
    FillInError::Store(store) => return ToolError::from(store),
    FillInError::PersonOnly => return person_only("teamFillIns.errors.personOnly"),
    FillInError::NotTeamProject => "team_project_required",
    FillInError::NotTeamResource { .. } => "not_team_resource",
    FillInError::ConnectionNotFound(_) => "connection_not_found",
    FillInError::ConnectionDoesNotFit { .. } => "connection_does_not_fit",
    FillInError::AccountNotFound(_) => "account_not_found",
    FillInError::AccountOfOtherCli(_) => "account_of_other_cli",
    FillInError::ProgramNotAbsolute => "program_not_absolute",
    FillInError::ProgramNotAFile => "program_not_a_file",
    FillInError::RemoteServer(_) => "remote_server",
  };
  ToolError::refused(code, vec![error.message()])
}
