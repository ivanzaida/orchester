//! `add_mcp_server`, `update_mcp_server` and `remove_mcp_server` in a team project act on the team's MCP server
//! resources (`docs/design/team-configuration.md`, "Who owns what"), as the team server was read this session
//! (`docs/design/team-configuration-live.md`), which only an admin changes: a member is refused `not_admin` before
//! anything else. Adding creates v1 on the team server, and updating publishes the next version there, at once
//! ([`super::created`]): the server's answer is the tool's, and while it cannot be reached nothing is saved. Removing
//! reads what uses the resource and answers `person_only` with that preview (decision 11): archiving stops runs on
//! every computer, so a person confirms it in the window ([`NotArchived::PersonOnly`]). A new server, and a version
//! that widens what agents may do ([`team_mcp_body::widenings`]), are a person's decision unless a person lets
//! automation change agent permissions. A `program` here is the body's program name (`npx`), team data like the rest,
//! so it widens like any other change; an absolute `program` is the program's path on this computer, kept here by
//! `team_fill_ins` under the same rule (`program_here` in the answer): an update publishes nothing for it, and a new
//! server publishes the file's name as its program. Local projects keep `mcp_servers`' own path: their MCP
//! servers are plain rows of the project, with no versions.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{ResourceKind, TeamMcpServer},
};
use orchester_store::{ProjectDb, StoreError};
use serde_json::{Value, json};

use super::{
  created::{create_team, refused},
  mcp_server_input::{AddMcpServer, UpdateMcpServer},
  team_fill_ins::{self, team_named},
  team_mcp_body as body, team_mcp_names, widening,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
    tools::team::{self, Team},
  },
  messages::Message,
  sharing::{
    NotArchived,
    sync::ArchiveError,
    team_edit::{NewTeamResource, Publication},
  },
  tasks::{detail::run_store_error, outside::Outside},
  team_config::{TeamConfig, TeamResource},
  team_fill_ins::FillInError,
  used_by::{UsedByError, used_by},
};

/// The team MCP server `named` (by id or name) of project `project`; `None` in a local project and for a name that is
/// none of the team's.
pub(super) async fn named(services: &Services, project: &str, named: &str) -> Result<Option<Found>, ToolError> {
  let Some(team) = team::of(services, project).await? else {
    return Ok(None);
  };
  let found = team_named(services, &team.open.repo_root, ResourceKind::McpServer, named)?;
  Ok(found.map(|resource| Found { team, resource }))
}

/// A team MCP server of a team project.
pub(super) struct Found {
  team: Team,
  resource: TeamResource,
}

/// Whether project `project` is a team project.
pub(super) async fn is_team(services: &Services, project: &str) -> Result<bool, ToolError> {
  Ok(team::of(services, project).await?.is_some())
}

pub(super) async fn add(services: &Services, mut args: AddMcpServer) -> Result<Value, ToolError> {
  let here = program_here(&mut args.fields.program)?;
  let blocked = services.clone();
  let (open, server) = blocking(move || {
    let open = blocked.project(&args.project)?;
    require_admin(&ProjectDb::open(&open.repo_root)?)?;
    let config = super::team_fill_ins::team_config(&blocked, &open.repo_root)?;
    let server = body::new_server(args.fields, &team_mcp_names(&config))?;
    let added = Message::new("appMcp.widens.mcpServerAdded").with("server", server.name().as_str().to_owned());
    widening::allowed(&blocked, vec![added])?;
    Ok((open, server))
  })
  .await?;
  let name = Name::new(server.name().as_str()).expect("an MCP server's name is a valid name");
  let new = NewTeamResource {
    id: ResourceId::generate(),
    kind: ResourceKind::McpServer,
    name,
    publication: publication(&server),
  };
  let id = create_team(services, &open.repo_root, new).await?;
  services.changes.record(open.project.id(), Outside::Connections);
  if let Some(path) = here.clone() {
    // Taken under the rule `widening::allowed` judged above, for a file checked to be there; what still fails here is
    // the tool's answer, the server being made.
    let (services, root) = (services.clone(), open.repo_root.clone());
    blocking(move || team_fill_ins::set_program(&services, &root, id, &path)).await?;
  }
  Ok(answered(server_json(id, &server, 1), here.as_deref(), false))
}

/// An absolute `program` of a new team server: the team holds the file's name, which each computer looks up on its
/// search path or sets, and this computer keeps the path. It must be a file here, checked before anything is made.
fn program_here(program: &mut Option<String>) -> Result<Option<PathBuf>, ToolError> {
  let Some(path) = program
    .take_if(|given| Path::new(given).is_absolute())
    .map(PathBuf::from)
  else {
    return Ok(None);
  };
  if !path.is_file() {
    return Err(team_fill_ins::refusal(FillInError::ProgramNotAFile));
  }
  let name = path
    .file_name()
    .and_then(|name| name.to_str())
    .expect("a file's path given as text ends in a name that is text");
  *program = Some(name.to_owned());
  Ok(Some(path))
}

/// The tools' answer: `server`, the program path this computer keeps for it when one was given, and whether nothing
/// changed at all.
fn answered(server: Value, here: Option<&Path>, unchanged: bool) -> Value {
  let mut answer = json!({});
  answer["server"] = server;
  if let Some(path) = here {
    answer["program_here"] = json!(path.display().to_string());
  }
  if unchanged {
    answer["unchanged"] = json!(true);
  }
  answer
}

pub(super) async fn update(services: &Services, found: Found, mut args: UpdateMcpServer) -> Result<Value, ToolError> {
  let Found { team, resource } = found;
  // The team holds a program's name only, so an absolute path can only be this computer's: it is kept here, for every
  // project of the team server, and publishes nothing.
  let here = args
    .fields
    .program
    .take_if(|program| Path::new(program).is_absolute())
    .map(PathBuf::from);
  if let Some(path) = here.clone() {
    let (services, root, server) = (services.clone(), team.open.repo_root.clone(), resource.id);
    blocking(move || team_fill_ins::set_program(&services, &root, server, &path)).await?;
  }
  let (blocked, root) = (services.clone(), team.open.repo_root.clone());
  let latest = latest_of(&resource)?;
  let before = latest.clone();
  let next = blocking(move || {
    let next = body::changed_server(&before, args.fields)?;
    // Only a change of the team's version is an Admin's; a program path alone was kept here above.
    if next != before {
      require_admin(&ProjectDb::open(&root)?)?;
      widening::allowed(&blocked, body::widenings(&before, &next))?;
    }
    Ok(next)
  })
  .await?;
  let number = resource.number().map_or(0, |number| number.get());
  if next == latest {
    let server = server_json(resource.id, &latest, number);
    return Ok(answered(server, here.as_deref(), here.is_none()));
  }
  let published = services
    .servers
    .publish_team(
      &team.open.repo_root,
      resource.id,
      (resource.number(), publication(&next)),
    )
    .await
    .map_err(|error| refused(&error))?;
  services.changes.record(team.open.project.id(), Outside::Connections);
  Ok(answered(
    server_json(resource.id, &next, published.get()),
    here.as_deref(),
    false,
  ))
}

pub(super) async fn remove(services: &Services, found: Found) -> Result<Value, ToolError> {
  let Found { team, resource } = found;
  let (root, id) = (team.open.repo_root.clone(), resource.id);
  let checked = root.clone();
  let config = services.workspace().team_read(&root).loaded()?;
  let stopping = blocking(move || {
    require_admin(&ProjectDb::open(&checked)?)?;
    stopping_here(&checked, id, &config)
  })
  .await?;
  let session = team.session(services).await?;
  let used = used_by(&root, Some(&session), id)
    .await
    .map_err(|error| used_by_refused(&error))?;
  Err(ToolError::previewed(
    "person_only",
    vec![NotArchived::person_only_message()],
    json!({
      "resource": { "id": id.to_string(), "name": resource.name.as_str(), "team": true },
      "used_by": serde_json::to_value(&used).expect("what uses a resource is plain data and serializes"),
      "runs_stopping_here": stopping,
    }),
  ))
}

/// Refused as `not_admin` unless the person may change the team's configuration.
fn require_admin(db: &ProjectDb) -> Result<(), ToolError> {
  if db.may_change_configuration()? {
    Ok(())
  } else {
    Err(StoreError::NotAdmin.into())
  }
}

/// The latest version of team MCP server `resource`, which a next one is made on.
fn latest_of(resource: &TeamResource) -> Result<TeamMcpServer, ToolError> {
  match resource.body::<TeamMcpServer>() {
    Some(Ok(body)) => Ok(body),
    Some(Err(_)) => Err(ToolError::refused(
      "not_team_form",
      vec![Message::new("appMcp.errors.teamMcpNotTeamForm")],
    )),
    None => Err(StoreError::NotPublished(resource.id).into()),
  }
}

/// `server` as the team holds it: it names no other team resource, secret by reference or CLI kind.
fn publication(server: &TeamMcpServer) -> Publication {
  Publication {
    body: serde_json::to_value(server).expect("a team MCP server serializes"),
    references: Vec::new(),
  }
}

/// Team MCP server `id` as the tools answer: `server` as its version `version` holds it.
fn server_json(id: ResourceId, server: &TeamMcpServer, version: u32) -> Value {
  let mut json = body::server_json(server);
  json["id"] = json!(id.to_string());
  json["team"] = json!(true);
  json["published_version"] = json!(version);
  json["resource_name"] = json!(server.name().as_str());
  json
}

/// The runs of this computer that pin team MCP server `id` and will be cancelled when the archive reaches it.
fn stopping_here(root: &Path, id: ResourceId, team: &TeamConfig) -> Result<Vec<Value>, ToolError> {
  let pinning = crate::sharing::sync::pinning(root, id, Some(team)).map_err(|error| match error {
    ArchiveError::Store(error) => ToolError::from(error),
    ArchiveError::Runs(error) => ToolError::from(run_store_error(error)),
  })?;
  Ok(
    pinning
      .iter()
      .flat_map(|pinning| pinning.runs())
      .map(|(run, task)| json!({ "run": run.to_string(), "task": task.to_string() }))
      .collect(),
  )
}

/// What uses the server could not be read: nothing is archived, since the answer must say what stops.
fn used_by_refused(error: &UsedByError) -> ToolError {
  match error {
    UsedByError::Team(read) => team::read_refused(read),
    UsedByError::NotFound => ToolError::not_found(Missing::McpServer, "the team's MCP server"),
    error => {
      tracing::warn!(%error, "what uses a team MCP server could not be read");
      ToolError::refused("team_read_failed", vec![Message::new("appMcp.errors.teamReadFailed")])
    }
  }
}
