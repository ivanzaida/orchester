//! Which local connection each agent profile of a team task's pipeline runs on, for its start's choices. The profiles
//! and the CLI each runs are read from the team server: the pipeline's latest published version names its profiles,
//! and each profile's latest version names its connection by a reference whose role is the CLI the Admin chose. A
//! profile runs on the connection the caller binds it to, otherwise on the only local agent connection of its CLI;
//! with none or several, the start is refused naming the profiles that need a choice. Nothing else is guessed: the
//! bindings are checked again against the bundle `work.start` answers, and a pipeline changed in between fails the
//! start there.

use orchester_api::{ReferenceTarget, SharedVersion, v2::PageRequest};
use orchester_core::{
  connection::{AgentProvider, Connection, ConnectionKind},
  id::ResourceId,
  resource::{AgentProfile, Pipeline, StepKind},
  run::inputs::{LocalBindings, ProfileBinding},
  sharing::ConnectionRole,
};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;

use super::{read, read_refused};
use crate::{
  app_mcp::error::{Missing, ToolError},
  messages::Message,
  team_reads::TeamProject,
};

/// The local connection a team profile runs on, chosen by the caller.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct Binding {
  /// The team agent profile's id.
  pub profile: String,
  /// A local agent connection's id, or its name.
  pub connection: String,
}

/// One agent profile the pipeline runs, as its latest version names it.
struct Profile {
  id: ResourceId,
  role: String,
  cli: AgentProvider,
}

/// The bindings of every agent profile of `pipeline`'s latest published version, on `connections`.
pub(super) async fn bind(
  project: &TeamProject,
  pipeline: ResourceId,
  connections: &[Connection],
  given: &[Binding],
) -> Result<LocalBindings, ToolError> {
  let mut profiles = Vec::new();
  for id in profiles_of(project, pipeline).await? {
    profiles.push(profile(project, id).await?);
  }
  let mut bound = Vec::new();
  let mut unbound = Vec::new();
  for profile in &profiles {
    match chosen(profile, connections, given)? {
      Some(connection) => bound.push(ProfileBinding {
        resource: profile.id,
        connection,
        account: None,
      }),
      None => unbound.push(profile.role.clone()),
    }
  }
  if !unbound.is_empty() {
    return Err(ToolError::refused(
      "bindings_required",
      vec![Message::new("appMcp.errors.bindingsRequired").with("profiles", unbound.join(", "))],
    ));
  }
  Ok(LocalBindings {
    profiles: bound,
    references: Vec::new(),
  })
}

/// The connection `profile` runs on: the caller's binding, else the only local connection of its CLI.
fn chosen(
  profile: &Profile,
  connections: &[Connection],
  given: &[Binding],
) -> Result<Option<orchester_core::id::ConnectionId>, ToolError> {
  if let Some(binding) = given.iter().find(|binding| binding.profile == profile.id.to_string()) {
    let lowered = binding.connection.to_lowercase();
    return connections
      .iter()
      .find(|connection| {
        connection.id().to_string() == binding.connection || connection.name().as_str().to_lowercase() == lowered
      })
      .map(|connection| Some(connection.id()))
      .ok_or_else(|| ToolError::not_found(Missing::Connection, binding.connection.clone()));
  }
  let mut matching = connections
    .iter()
    .filter(|connection| *connection.kind() == ConnectionKind::Agent(profile.cli));
  Ok(match (matching.next(), matching.next()) {
    (Some(only), None) => Some(only.id()),
    _ => None,
  })
}

/// The profiles the agent steps of `pipeline`'s latest version run, each once, in step order.
async fn profiles_of(project: &TeamProject, pipeline: ResourceId) -> Result<Vec<ResourceId>, ToolError> {
  let latest = latest(project, pipeline).await?;
  let body: Pipeline = serde_json::from_value(latest.body).map_err(|_| unreadable())?;
  let mut profiles = Vec::new();
  for step in body.steps() {
    if let StepKind::Agent {
      profile: Some(profile), ..
    } = &step.kind
      && !profiles.contains(profile)
    {
      profiles.push(*profile);
    }
  }
  Ok(profiles)
}

/// Profile `id` as its latest version names it, with the CLI its connection reference's role says.
async fn profile(project: &TeamProject, id: ResourceId) -> Result<Profile, ToolError> {
  let latest = latest(project, id).await?;
  let body: AgentProfile = serde_json::from_value(latest.body).map_err(|_| unreadable())?;
  let connection = body.connection();
  let cli = latest
    .references
    .iter()
    .find(|reference| reference.target == ReferenceTarget::Connection { id: connection })
    .and_then(|reference| reference.role.as_deref())
    .and_then(ConnectionRole::parse)
    .and_then(|role| match role {
      ConnectionRole::Agent(cli) => Some(cli),
      ConnectionRole::GitHub => None,
    })
    .ok_or_else(unreadable)?;
  Ok(Profile {
    id,
    role: body.role().as_str().to_owned(),
    cli,
  })
}

/// The latest published version of `resource`, the last of its versions, oldest first.
async fn latest(project: &TeamProject, resource: ResourceId) -> Result<SharedVersion, ToolError> {
  let mut page = PageRequest::first();
  let mut last = None;
  loop {
    let read = read(project, project.version_page(resource, page))
      .await
      .map_err(|error| read_refused(&error))?
      .into_value();
    last = read.items.into_iter().last().or(last);
    match read.next {
      Some(next) => page = PageRequest::new(Some(next), None).expect("the default page limit is in range"),
      None => break,
    }
  }
  last.ok_or_else(|| {
    ToolError::refused(
      "pipeline_not_published",
      vec![Message::new("appMcp.errors.teamNotPublished")],
    )
  })
}

/// A version the team server answered that this version cannot read as its kind.
fn unreadable() -> ToolError {
  ToolError::refused("team_read_failed", vec![Message::new("appMcp.errors.teamReadFailed")])
}
