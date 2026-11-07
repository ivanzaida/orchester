//! What this computer chooses for a team task's start, the same for the MCP tool `run_task` and the Task page's Run
//! (ORC-725's rule): the commit the module's branch is at here, the start's base; and which local connection each agent
//! profile of the task's pipeline runs on. The profiles and the CLI each runs are read from the team server: the
//! pipeline's latest published version names its profiles, and each profile's latest version names its connection by a
//! reference whose role is the CLI the Admin chose. A profile runs on the connection the caller binds it to, otherwise
//! on the connection its reference names when this computer has it with that very id and CLI (the publisher's own
//! computer), otherwise on the only local agent connection of its CLI; with none or several, the start is refused
//! naming the profiles that need a choice. Each MCP server a profile's version names runs as the local server a person
//! bound it to, else as this computer's own server with that id; with neither, the start is refused naming the servers
//! ([`McpHere`]). Nothing else is guessed: the bindings are checked again against the bundle
//! `work.start` answers, and a pipeline changed in between fails the start there.

use std::path::Path;

use orchester_api::{ReferenceTarget, SharedVersion, v2::PageRequest};
use orchester_core::{
  command::CommandRunner,
  connection::{AgentProvider, Connection, ConnectionKind, mcp::McpServer},
  id::{ConnectionId, ResourceId},
  integration::base::set_branch_tip,
  resource::{AgentProfile, Pipeline, StepKind},
  run::{
    BranchName, CommitId,
    inputs::{BoundReference, LocalBindings, ProfileBinding},
  },
  sharing::ConnectionRole,
  store::McpServerStore,
};
use orchester_store::{BoundReference as StoredReference, ProjectDb, StoreError};

use crate::{
  tasks::changes::GIT,
  team_reads::{TeamProject, TeamReadError},
};

/// Why a start's choices cannot be made here.
#[derive(Debug, thiserror::Error)]
pub enum ChoiceError {
  /// The branch is not in this checkout, or git cannot read its tip.
  #[error("branch {0} cannot be read here")]
  BranchNotFound(String),
  /// The team server could not read a pipeline or profile version.
  #[error(transparent)]
  Read(#[from] TeamReadError),
  /// The pipeline or a profile has no published version.
  #[error("a pipeline or profile of the start has no published version")]
  NotPublished,
  /// A version the team server answered cannot be read as its kind.
  #[error("a version the team server answered cannot be read")]
  Unreadable,
  /// A connection the caller named is not one of this computer's.
  #[error("no connection {0} here")]
  ConnectionNotFound(String),
  /// These profiles, by role, have no connection of their CLI here, or several: the caller chooses.
  #[error("the profiles {0:?} need a connection chosen")]
  BindingsRequired(Vec<String>),
  /// These MCP servers, by name, that the pipeline's profiles use are not set up on this computer: nothing starts
  /// without them.
  #[error("the MCP servers {0:?} are not set up here")]
  McpServersRequired(Vec<String>),
}

/// This computer's MCP servers as a team start gives them to the profiles that name them: a person's binding of the
/// publisher's server first, else this computer's own server with the very id the version names (the publisher's own
/// computer). Nothing is matched by name.
#[derive(Debug, Default)]
pub struct McpHere {
  servers: Vec<ConnectionId>,
  bound: Vec<(ConnectionId, ConnectionId)>,
}

impl McpHere {
  /// The MCP servers of the project kept in `db` and the person's bindings of the publishers' ones.
  pub fn read(db: &ProjectDb) -> Result<Self, StoreError> {
    let servers = db.mcp_servers()?.iter().map(McpServer::id).collect();
    let bound = db
      .reference_bindings()?
      .into_iter()
      .filter_map(|kept| match kept.reference {
        StoredReference::McpServer {
          reference,
          bound: Some(bound),
        } => Some((reference, bound.local)),
        StoredReference::McpServer { bound: None, .. }
        | StoredReference::Connection { .. }
        | StoredReference::Secret { .. } => None,
      })
      .collect();
    Ok(Self { servers, bound })
  }

  /// The local server the publisher's server `named` runs as here, if any.
  fn local(&self, named: ConnectionId) -> Option<ConnectionId> {
    self
      .bound
      .iter()
      .find(|(reference, _)| *reference == named)
      .map(|(_, local)| *local)
      .or_else(|| self.servers.contains(&named).then_some(named))
  }
}

/// The commit `branch` is at in the checkout at `root`: a module's branch is a start's base and a delivery's source.
pub fn branch_tip(
  runner: &(dyn CommandRunner + Send + Sync),
  root: &Path,
  branch: &str,
) -> Result<CommitId, ChoiceError> {
  let missing = || ChoiceError::BranchNotFound(branch.to_owned());
  let name = BranchName::new(branch).map_err(|_| missing())?;
  let revision = set_branch_tip(&runner, Path::new(GIT), (root, &name)).map_err(|error| {
    tracing::warn!(%error, "cannot read a team branch's tip");
    missing()
  })?;
  CommitId::new(revision.as_str()).map_err(|_| missing())
}

/// One agent profile the pipeline runs, as its latest version names it.
struct Profile {
  id: ResourceId,
  role: String,
  cli: AgentProvider,
  /// The connection its version's reference names, by its id on the publisher's computer.
  connection: ConnectionId,
  /// The MCP servers its version's references name, by their id on the publisher's computer, with their names.
  mcp_servers: Vec<(ConnectionId, String)>,
}

/// The bindings of every agent profile of `pipeline`'s latest published version on `connections`, the caller's
/// `given` first: each `(profile id, connection id or name)`, and of the MCP servers those profiles name, on `mcp`.
/// The versions are read from `project`, whose subscription the caller follows meanwhile: its reads wait for the
/// subscription's position.
pub async fn bind(
  project: &TeamProject,
  pipeline: ResourceId,
  (connections, mcp): (&[Connection], &McpHere),
  given: &[(String, String)],
) -> Result<LocalBindings, ChoiceError> {
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
        required_cli: None,
      }),
      None => unbound.push(profile.role.clone()),
    }
  }
  if !unbound.is_empty() {
    return Err(ChoiceError::BindingsRequired(unbound));
  }
  Ok(LocalBindings {
    profiles: bound,
    references: mcp_references(&profiles, mcp)?,
  })
}

/// Each MCP server the profiles name, once, bound to the local server it runs as here; refused naming every one that
/// has none, so a run never starts silently without a server its profiles use.
fn mcp_references(profiles: &[Profile], mcp: &McpHere) -> Result<Vec<BoundReference>, ChoiceError> {
  let mut references = Vec::new();
  let mut missing = Vec::new();
  for (named, name) in profiles.iter().flat_map(|profile| &profile.mcp_servers) {
    match mcp.local(*named) {
      Some(local) => {
        let taken = references.iter().any(|reference| match reference {
          BoundReference::McpServer {
            named: other,
            local: kept,
          } => other == named || *kept == local,
          BoundReference::Connection { .. } | BoundReference::Secret { .. } => false,
        });
        if !taken {
          references.push(BoundReference::McpServer { named: *named, local });
        }
      }
      None if !missing.contains(name) => missing.push(name.clone()),
      None => {}
    }
  }
  if missing.is_empty() {
    Ok(references)
  } else {
    Err(ChoiceError::McpServersRequired(missing))
  }
}

/// The connection `profile` runs on: the caller's binding, else the connection its reference names when this computer
/// has it with that id and CLI, else the only local connection of its CLI.
fn chosen(
  profile: &Profile,
  connections: &[Connection],
  given: &[(String, String)],
) -> Result<Option<ConnectionId>, ChoiceError> {
  if let Some((_, named)) = given.iter().find(|(id, _)| *id == profile.id.to_string()) {
    let lowered = named.to_lowercase();
    return connections
      .iter()
      .find(|connection| connection.id().to_string() == *named || connection.name().as_str().to_lowercase() == lowered)
      .map(|connection| Some(connection.id()))
      .ok_or_else(|| ChoiceError::ConnectionNotFound(named.clone()));
  }
  let runs_cli = |connection: &&Connection| *connection.kind() == ConnectionKind::Agent(profile.cli);
  if let Some(named) = connections
    .iter()
    .filter(runs_cli)
    .find(|connection| connection.id() == profile.connection)
  {
    return Ok(Some(named.id()));
  }
  let mut matching = connections.iter().filter(runs_cli);
  Ok(match (matching.next(), matching.next()) {
    (Some(only), None) => Some(only.id()),
    _ => None,
  })
}

/// The profiles the agent steps of `pipeline`'s latest version run, each once, in step order.
async fn profiles_of(project: &TeamProject, pipeline: ResourceId) -> Result<Vec<ResourceId>, ChoiceError> {
  let latest = latest(project, pipeline).await?;
  let body: Pipeline = serde_json::from_value(latest.body).map_err(|_| ChoiceError::Unreadable)?;
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
async fn profile(project: &TeamProject, id: ResourceId) -> Result<Profile, ChoiceError> {
  let latest = latest(project, id).await?;
  let body: AgentProfile = serde_json::from_value(latest.body).map_err(|_| ChoiceError::Unreadable)?;
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
    .ok_or(ChoiceError::Unreadable)?;
  let mcp_servers = latest
    .references
    .iter()
    .filter_map(|reference| match reference.target {
      ReferenceTarget::McpServer { id } => Some((id, reference.name.clone())),
      ReferenceTarget::Connection { .. } | ReferenceTarget::Secret { .. } | ReferenceTarget::Tool { .. } => None,
    })
    .collect();
  Ok(Profile {
    id,
    role: body.role().as_str().to_owned(),
    cli,
    connection,
    mcp_servers,
  })
}

/// The latest published version of `resource`, the last of its versions, oldest first.
async fn latest(project: &TeamProject, resource: ResourceId) -> Result<SharedVersion, ChoiceError> {
  let mut page = PageRequest::first();
  let mut last = None;
  loop {
    let read = project.version_page(resource, page).await?.into_value();
    last = read.items.into_iter().last().or(last);
    match read.next {
      Some(next) => page = PageRequest::new(Some(next), None).expect("the default page limit is in range"),
      None => break,
    }
  }
  last.ok_or(ChoiceError::NotPublished)
}
