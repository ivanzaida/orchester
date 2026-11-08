//! `add_mcp_server`, `update_mcp_server` and `remove_mcp_server` in a team project act on the team's MCP server
//! resources (`docs/design/team-configuration.md`, "Who owns what"), which only an admin changes: a member is refused
//! `not_admin` before anything else. Adding creates v1 on the team server ([`create_published_server`], settled as
//! every new team resource is, [`settle`]); updating saves a draft and publishes the next version at once; removing
//! archives the resource on the server after reading what uses it (decision 11: never refused for being used). A new
//! server, and a version that widens what agents may do ([`team_mcp_body::widenings`]), are a person's decision
//! unless a person lets automation change agent permissions. A `program` here is the body's program name (`npx`), team
//! data like the rest, so it widens like any other change; the program's path on this computer is a person's alone
//! (`team_fill_ins`, in the window) and no tool sets it. Local projects keep `mcp_servers`' own path: their MCP servers
//! are plain rows of the project, with no versions.

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, Resource, ResourceKind, Slug, TeamMcpServer, Version},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, PublishState, StoreError};
use serde_json::{Value, json};

use super::{
  created::{settle, with_waiting},
  mcp_server_input::{AddMcpServer, ServerFields, UpdateMcpServer},
  team_fill_ins::team_resource_named,
  team_mcp_body as body, widening,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
    tools::team::{self, Team},
  },
  messages::Message,
  sharing::{NotArchived, sync::ArchiveError},
  tasks::{detail::run_store_error, outside::Outside},
  used_by::{UsedByError, used_by},
};

/// A slug for a server of a name no resource has made one of.
const FALLBACK_SLUG: &str = "mcp-server";

/// The team MCP server `named` (by id or name) of project `project`; `None` in a local project and for a name that is
/// none of the team's.
pub(super) async fn named(services: &Services, project: &str, named: &str) -> Result<Option<Found>, ToolError> {
  let Some(team) = team::of(services, project).await? else {
    return Ok(None);
  };
  let named = named.to_owned();
  blocking(move || {
    let db = ProjectDb::open(&team.open.repo_root)?;
    let found = match team_resource_named(&db, ResourceKind::McpServer, &named)? {
      Some(resource) => Some(resource),
      None => unsent_named(&db, &named)?,
    };
    Ok(found.map(|resource| Found { team, resource }))
  })
  .await
}

/// The team MCP server `named` (by id or name) that this computer made and the team server has not received: the tools
/// find it to say so ([`not_sent_yet`]), where the team's own are the ones the server has.
fn unsent_named(db: &ProjectDb, named: &str) -> Result<Option<Resource>, ToolError> {
  for resource in db.resources(ResourceKind::McpServer)? {
    if (resource.id().to_string() == named || resource.name().as_str() == named) && first_waits(db, resource.id())? {
      return Ok(Some(resource));
    }
  }
  Ok(None)
}

/// A team MCP server of a team project.
pub(super) struct Found {
  team: Team,
  resource: Resource,
}

/// Whether project `project` is a team project.
pub(super) async fn is_team(services: &Services, project: &str) -> Result<bool, ToolError> {
  Ok(team::of(services, project).await?.is_some())
}

pub(super) async fn add(services: &Services, args: AddMcpServer) -> Result<Value, ToolError> {
  let blocked = services.clone();
  let (open, id) = blocking(move || {
    let open = blocked.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    require_admin(&db)?;
    let server = body::new_server(args.fields, &team_names(&db)?)?;
    let added = Message::new("appMcp.widens.mcpServerAdded").with("server", server.name().as_str().to_owned());
    widening::allowed(&blocked, vec![added])?;
    let id = create_published_server(&mut db, &server, Timestamp::now())?;
    Ok((open, id))
  })
  .await?;
  // Settled before the change is recorded, so a screen that reads on it does not see a server the team server refused.
  let waiting = settle(services, open.repo_root.clone(), id).await;
  services.changes.record(open.project.id(), Outside::Connections);
  let waiting = waiting?;
  let server = blocking(move || published_json(&open.repo_root, id)).await?;
  Ok(with_waiting(json!({ "server": server }), "server", waiting))
}

pub(super) async fn update(services: &Services, found: Found, args: UpdateMcpServer) -> Result<Value, ToolError> {
  let (blocked, root) = (services.clone(), found.team.open.repo_root.clone());
  let (id, project) = (found.resource.id(), found.team.open.project.id());
  let published = blocking(move || publish_next(&blocked, &root, id, args.fields)).await?;
  let Some(published) = published else {
    let root = found.team.open.repo_root.clone();
    return Ok(json!({ "server": blocking(move || published_json(&root, id)).await?, "unchanged": true }));
  };
  let sent = services.servers.send_published(&found.team.open.repo_root).await;
  services.changes.record(project, Outside::Connections);
  let root = found.team.open.repo_root;
  let (standing, server) = blocking(move || {
    let standing = standing_of(&ProjectDb::open(&root)?, published)?;
    Ok((standing, published_json(&root, id)?))
  })
  .await?;
  if let Err(reason) = &sent {
    tracing::warn!(key = reason.key, "a team MCP server's new version waits to be sent");
  }
  match standing {
    Some(PublishState::Refused { code }) => Err(not_taken(&code)),
    Some(PublishState::Conflict { .. }) => Err(not_taken("version_conflict")),
    Some(PublishState::Waiting) => Ok(with_waiting(
      json!({ "server": server }),
      "server",
      Some(json!({ "code": "team_server_unreachable" })),
    )),
    None => Ok(json!({ "server": server })),
  }
}

pub(super) async fn remove(services: &Services, found: Found) -> Result<Value, ToolError> {
  let Found { team, resource } = found;
  let (root, id) = (team.open.repo_root.clone(), resource.id());
  let checked = root.clone();
  let stopping = blocking(move || {
    let db = ProjectDb::open(&checked)?;
    require_admin(&db)?;
    not_sent_yet(&db, id)?;
    stopping_here(&checked, id)
  })
  .await?;
  let session = team.session(services).await?;
  let used = used_by(&root, Some(&session), id)
    .await
    .map_err(|error| used_by_refused(&error))?;
  services
    .servers
    .archive_resource(&root, id)
    .await
    .map_err(|error| not_archived(&error))?;
  services.changes.record(team.open.project.id(), Outside::Connections);
  Ok(json!({
    "removed": { "id": id.to_string(), "name": resource.name().as_str(), "team": true, "archived": true },
    "used_by": serde_json::to_value(&used).expect("what uses a resource is plain data and serializes"),
    "runs_stopping_here": stopping,
  }))
}

/// Refused as `not_admin` unless the person may change the team's configuration.
fn require_admin(db: &ProjectDb) -> Result<(), ToolError> {
  if db.may_change_configuration()? {
    Ok(())
  } else {
    Err(StoreError::NotAdmin.into())
  }
}

/// Refused as `team_mcp_server_not_sent` while team MCP server `id` has only its first version, which the team server
/// has not numbered: it knows nothing of the server, so there is nothing to read the use of or to archive, and a next
/// version has nothing to be based on.
fn not_sent_yet(db: &ProjectDb, id: ResourceId) -> Result<(), ToolError> {
  if first_waits(db, id)? {
    return Err(ToolError::refused(
      "team_mcp_server_not_sent",
      vec![Message::new("appMcp.errors.teamMcpServerNotSent")],
    ));
  }
  Ok(())
}

/// Whether the first version of resource `id` waits in the outbox: the team server has not numbered it.
fn first_waits(db: &ProjectDb, id: ResourceId) -> Result<bool, StoreError> {
  Ok(
    db.waiting_publishes()?
      .iter()
      .any(|waiting| waiting.resource == id && waiting.based_on.is_none()),
  )
}

/// The names of the team's MCP servers, which a new one may not take.
pub(super) fn team_names(db: &ProjectDb) -> Result<Vec<orchester_core::connection::mcp::McpServerName>, StoreError> {
  Ok(
    db.resources(ResourceKind::McpServer)?
      .iter()
      // A resource named otherwise than an MCP server is can clash with no MCP server's name.
      .filter_map(|resource| orchester_core::connection::mcp::McpServerName::new(resource.name().as_str()).ok())
      .collect(),
  )
}

/// Creates the team MCP server `server` as its first version, v1 with no draft, under a slug made from its name. The
/// version waits for the team server ([`crate::sharing::Servers::send_created`]).
pub(super) fn create_published_server(
  db: &mut ProjectDb,
  server: &TeamMcpServer,
  now: Timestamp,
) -> Result<ResourceId, StoreError> {
  let name = Name::new(server.name().as_str()).expect("an MCP server's name is a valid name");
  let taken: Vec<Slug> = db.slugs(ResourceKind::McpServer)?;
  let fallback = Slug::new(FALLBACK_SLUG).expect("the fallback slug is valid");
  let slug = Slug::unique_for(&name, &fallback, &taken);
  Ok(db.create_published(slug, name, server, now)?.0.id())
}

/// Saves `fields` over the latest version of team MCP server `id` as the draft and publishes it as the next version,
/// when they change it and the gate lets it through; `None` when they change nothing.
fn publish_next(
  services: &Services,
  root: &std::path::Path,
  id: ResourceId,
  fields: ServerFields,
) -> Result<Option<orchester_core::id::VersionId>, ToolError> {
  let mut db = ProjectDb::open(root)?;
  require_admin(&db)?;
  not_sent_yet(&db, id)?;
  let latest = db
    .latest_version::<TeamMcpServer>(id)?
    .ok_or(StoreError::NotPublished(id))?;
  let next = body::changed_server(latest.body(), fields)?;
  if &next == latest.body() {
    return Ok(None);
  }
  widening::allowed(services, body::widenings(latest.body(), &next))?;
  let (number, now) = (latest.number(), Timestamp::now());
  db.update_draft::<TeamMcpServer>(id, |draft, used| {
    // A draft that differs from the published version is someone's unpublished work (the window's editor): saving
    // over it would lose it, and publishing it unreviewed is not this tool's to do.
    let draft_is_work = draft.as_ref().is_some_and(|draft| draft.body() != latest.body());
    if used.as_ref().map(Version::number) != Some(number) || draft_is_work {
      return Err(StoreError::DraftChanged(id));
    }
    Ok(Draft::new(next.clone(), Some(number), now))
  })?;
  Ok(Some(
    db.publish_draft_matching_version(id, &next, Some(number), now)?.id(),
  ))
}

/// How the version `version` published here stands with the team server: `None` once the server numbered it.
fn standing_of(db: &ProjectDb, version: orchester_core::id::VersionId) -> Result<Option<PublishState>, StoreError> {
  Ok(
    db.waiting_publishes()?
      .into_iter()
      .find(|waiting| waiting.version == version)
      .map(|waiting| waiting.state),
  )
}

/// Team MCP server `id` as the tools answer, with its resource and the version this computer uses.
fn published_json(root: &std::path::Path, id: ResourceId) -> Result<Value, ToolError> {
  let db = ProjectDb::open(root)?;
  let resource = db
    .resource(id)?
    .ok_or_else(|| ToolError::not_found(Missing::McpServer, id.to_string()))?;
  let latest = db
    .latest_version::<TeamMcpServer>(id)?
    .ok_or(StoreError::NotPublished(id))?;
  let mut server = body::server_json(latest.body());
  server["id"] = json!(id.to_string());
  server["team"] = json!(true);
  server["published_version"] = json!(latest.number().get());
  server["resource_name"] = json!(resource.name().as_str());
  Ok(server)
}

/// The runs of this computer that pin team MCP server `id` and will be cancelled when the archive reaches it.
fn stopping_here(root: &std::path::Path, id: ResourceId) -> Result<Vec<Value>, ToolError> {
  let pinning = crate::sharing::sync::pinning(root, id).map_err(|error| match error {
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

/// The team server did not take the version published here; it stays on this computer, unsent.
fn not_taken(code: &str) -> ToolError {
  ToolError::refused(
    "team_server_refused",
    vec![Message::new("appMcp.errors.teamServerRefusedVersion").with("code", code.to_owned())],
  )
}

fn not_archived(error: &NotArchived) -> ToolError {
  match error {
    NotArchived::Refused { code } => ToolError::refused(
      "team_server_refused",
      vec![Message::new("appMcp.errors.teamServerRefusedArchive").with("code", code.clone())],
    ),
    NotArchived::Failed(reason) => ToolError::refused("team_server_not_reached", vec![reason.clone()]),
  }
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
