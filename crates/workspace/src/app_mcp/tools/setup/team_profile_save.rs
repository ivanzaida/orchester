//! `save_profile_draft` in a team project: the team keeps no draft on a computer, so the fields the tool takes publish
//! the profile's next version on the team server at once, made on the version they were read from, as an Admin's Save
//! in the Agents hub does (`agents::team_body_edit`); without `profile`, the team's new profile as its version 1. Every
//! field is checked as for a local profile (`profile_input`), the MCP servers, instructions, skills and tools named
//! among the team's, and the profile's secrets kept. What it widens beyond the team's latest version (permissions,
//! another CLI, an MCP server added) and what changes its agents' prompts are a person's decision, as for a local
//! profile: `person_only` unless a person lets automation change agent permissions. The account it names is the one
//! this computer runs it on (`set_profile_account`). The team server decides who may publish (`not_admin`).

use orchester_core::{
  connection::{invocation::Platform, mcp::McpServerSelection},
  id::ResourceId,
  name::Name,
  permission::AgentPermissions,
  resource::{
    AgentProfile, ChosenResources, ResourceKind, TeamAgentProfile, TeamCounterparts, TeamProfileError, VersionNumber,
  },
  store::{ConnectionStore, McpServerStore},
};
use orchester_store::{ProjectDb, StoreError};
use serde_json::Value;

use super::{
  created::{create_team, refused as edit_refused},
  profile_input::{SaveProfileDraft, account_connection, form, offered, permissions, refused, sandbox_offered},
  runs_on::{SetProfileAccount, set_profile_account},
  team_profiles::{
    TeamProfile, TeamProject, local_form, named, resource_named, server_named, shown_now, team_server_ids,
  },
  widening,
};
use crate::{
  agents::{local_edit::GivenKind, team_body_edit, widening::between},
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
  prompt_changes,
  sharing::team_edit::{EditRefused, Local, NewTeamResource, Publication, Refusal},
  tasks::outside::Outside,
};

/// What a save publishes: the profile in the team's form, made on its latest version (none for a new profile).
struct Prepared {
  latest: Option<(ResourceId, VersionNumber)>,
  team: TeamAgentProfile,
  role: Name,
}

/// Publishes the profile `args` give in the team project `team` as its next version, or as a new team profile; the
/// profile as `get_profile` shows it after.
pub(super) async fn save(services: &Services, team: &TeamProject, args: SaveProfileDraft) -> Result<Value, ToolError> {
  let (reading, config, root) = (services.clone(), team.config.clone(), team.open.repo_root.clone());
  let (project, account) = (args.project.clone(), args.account.clone());
  let Prepared {
    latest,
    team: body,
    role,
  } = blocking(move || prepared(&reading, (&root, &config), &args)).await?;
  let root = &team.open.repo_root;
  let id = match latest {
    Some((id, base)) => {
      team_body_edit::save_team_profile(&services.servers, root, (id, base), &body)
        .await
        .map_err(|error| edit_refused(&error))?;
      id
    }
    None => {
      let publication = Publication::of_team_profile(&body).map_err(|error| edit_refused(&error))?;
      let new = NewTeamResource {
        id: ResourceId::generate(),
        kind: ResourceKind::AgentProfile,
        name: role,
        publication,
      };
      create_team(services, root, new).await?
    }
  };
  services.changes.record(team.open.project.id(), Outside::Profiles);
  // The account it was saved with is the one this computer runs it on, as its Runs on would choose it.
  let runs_on = SetProfileAccount {
    project,
    profile: id.to_string(),
    account,
  };
  set_profile_account(services, runs_on).await?;
  shown_now(services, &team.open, id).await
}

/// The profile `args` give as the team's next version of the profile they name, checked as the editor checks it and
/// refused where it widens or changes prompts without a person's leave. Blocks on SQLite.
fn prepared(
  services: &Services,
  (root, config): (&std::path::Path, &crate::team_config::TeamConfig),
  args: &SaveProfileDraft,
) -> Result<Prepared, ToolError> {
  let latest = args.profile.as_deref().map(|name| named(config, name)).transpose()?;
  match (&latest, args.expected_revision.as_deref()) {
    (Some(latest), Some(expected)) if expected != latest.number.get().to_string() => {
      return Err(StoreError::DraftChanged(latest.id).into());
    }
    (None, Some(_)) => {
      return Err(ToolError::InvalidArguments(
        "expected_revision names a profile's version; a new profile has none".into(),
      ));
    }
    _ => {}
  }
  let connection = account_connection(services, root, args)?;
  let base = latest.as_ref().map(|latest| local_form(latest, Some(&connection)));
  let most_restrictive = AgentPermissions::most_restrictive(connection.provider(), Platform::current());
  let base_permissions = base.as_ref().map_or(&most_restrictive, AgentProfile::permissions);
  let chosen = match &args.permissions {
    Some(given) => permissions(given, root, base.as_ref().map(AgentProfile::permissions))?,
    None => base_permissions.clone(),
  };
  let given_sandbox = args.permissions.as_ref().and_then(|given| given.codex_sandbox);
  let chosen = sandbox_offered(chosen, &connection, (given_sandbox, true))?;
  offered(&connection, &chosen, args)?;
  let servers = selection(config, &args.mcp_servers)?;
  let given = given(config, args, base.as_ref())?;
  let profile = form(args, &connection, (&chosen, &servers), &given)?;
  let none = McpServerSelection::none();
  let base_servers = base.as_ref().map_or(&none, AgentProfile::mcp_servers);
  let base_cli = latest
    .as_ref()
    .map_or(connection.provider(), |latest| latest.body.cli());
  let reasons = between(
    (base_permissions, base_servers),
    (base_cli, connection.provider()),
    &profile,
  );
  widening::check_reasons(services, reasons)?;
  widening::prompt_allowed(services, prompt_changes::profile(base.as_ref(), &profile))?;
  let db = ProjectDb::open(root)?;
  Ok(Prepared {
    team: team_form(&db, config, &profile, latest.as_ref())?,
    latest: latest.map(|latest| (latest.id, latest.number)),
    role: profile.role().clone(),
  })
}

/// The team MCP servers `named` names.
fn selection(config: &crate::team_config::TeamConfig, named: &[String]) -> Result<McpServerSelection, ToolError> {
  let ids = named
    .iter()
    .map(|name| server_named(config, name))
    .collect::<Result<Vec<_>, _>>()?;
  McpServerSelection::new(ids).map_err(|_| {
    let reason = Message::new("appMcp.errors.mcpServersInvalid").with("max", McpServerSelection::MAX.to_string());
    refused("invalid_mcp_servers", reason)
  })
}

/// The team's instructions, skills and tools the profile gives its agents: each list given, named among the team's
/// resources of its kind; one left out is `kept`'s, a new profile's empty.
fn given(
  config: &crate::team_config::TeamConfig,
  args: &SaveProfileDraft,
  kept: Option<&AgentProfile>,
) -> Result<[ChosenResources; 3], ToolError> {
  let list = |named: Option<&Vec<String>>, kind: GivenKind, missing: Missing| -> Result<ChosenResources, ToolError> {
    let Some(named) = named else {
      return Ok(kept.map_or(ChosenResources::EMPTY, |profile| kind.of(profile).clone()));
    };
    let ids = named
      .iter()
      .map(|name| resource_named(config, kind.resource_kind(), name, missing))
      .collect::<Result<Vec<_>, _>>()?;
    ChosenResources::new(ids).map_err(|_| refused("invalid_given", Message::new("appMcp.errors.givenTwice")))
  };
  Ok([
    list(
      args.instruction_resources.as_ref(),
      GivenKind::Instruction,
      Missing::Instruction,
    )?,
    list(args.skills.as_ref(), GivenKind::Skill, Missing::Skill)?,
    list(args.tools.as_ref(), GivenKind::Tool, Missing::Tool)?,
  ])
}

/// `profile` in the team's form, with the secrets of its `latest` version, which a local profile does not name.
fn team_form(
  db: &ProjectDb,
  config: &crate::team_config::TeamConfig,
  profile: &AgentProfile,
  latest: Option<&TeamProfile>,
) -> Result<TeamAgentProfile, ToolError> {
  let (connections, local_mcp_servers) = (db.connections()?, db.mcp_servers()?);
  let team_mcp_servers = team_server_ids(config);
  let team = TeamAgentProfile::from_local(
    profile,
    TeamCounterparts {
      connections: &connections,
      local_mcp_servers: &local_mcp_servers,
      team_mcp_servers: &team_mcp_servers,
    },
  )
  .map_err(not_team)?;
  match latest {
    Some(latest) => team.with_secrets(latest.body.secrets().to_vec()).map_err(not_team),
    None => Ok(team),
  }
}

/// Why `profile` cannot be the team's, as the hub refuses it.
fn not_team(error: TeamProfileError) -> ToolError {
  let local = match error {
    TeamProfileError::LocalMcpServer(_) => Local::Entities,
    TeamProfileError::WritableFolders { .. } => Local::WritableFolders,
    other => return ToolError::InvalidArguments(other.to_string()),
  };
  edit_refused(&EditRefused::Refused(Refusal::Local(local)))
}
