//! What this computer chooses for a team task's start, the same for the MCP tool `run_task` and the Task page's Run
//! (ORC-725's rule): the commit the module's branch is at here, the start's base; and which local connection each agent
//! profile of the task's pipeline runs on. The profiles and the CLI each runs are read from the team server: the
//! pipeline's latest published version names its profiles, and each profile's latest version names its connection by a
//! reference whose role is the CLI the Admin chose. A profile runs on the connection the caller binds it to, otherwise
//! on the only local agent connection of its CLI; with none or several, the start is refused naming the profiles that
//! need a choice. Nothing else is guessed: the bindings are checked again against the bundle `work.start` answers, and
//! a pipeline changed in between fails the start there.

use std::path::Path;

use orchester_api::{ReferenceTarget, SharedVersion, v2::PageRequest};
use orchester_core::{
  command::CommandRunner,
  connection::{AgentProvider, Connection, ConnectionKind},
  id::{ConnectionId, ResourceId},
  integration::base::set_branch_tip,
  resource::{AgentProfile, Pipeline, StepKind},
  run::{
    BranchName, CommitId,
    inputs::{LocalBindings, ProfileBinding},
  },
  sharing::ConnectionRole,
};

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
}

/// The bindings of every agent profile of `pipeline`'s latest published version on `connections`, the caller's
/// `given` first: each `(profile id, connection id or name)`. The versions are read from `project`, whose subscription
/// the caller follows meanwhile: its reads wait for the subscription's position.
pub async fn bind(
  project: &TeamProject,
  pipeline: ResourceId,
  connections: &[Connection],
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
    references: Vec::new(),
  })
}

/// The connection `profile` runs on: the caller's binding, else the only local connection of its CLI.
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
  let mut matching = connections
    .iter()
    .filter(|connection| *connection.kind() == ConnectionKind::Agent(profile.cli));
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
  Ok(Profile {
    id,
    role: body.role().as_str().to_owned(),
    cli,
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
