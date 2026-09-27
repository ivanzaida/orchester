//! Reading, saving and publishing one pipeline through the project database, and what its steps
//! can be assigned to. Every function blocks on SQLite; the UI calls them off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::{AgentProvider, ConnectionKind},
  id::ResourceId,
  permission::AgentPermissions,
  resource::{
    AgentProfile, Draft, DraftSaved, ModelId, Pipeline, ResourceKind, Tool, ToolAction, ToolKind, Version,
    VersionNumber,
  },
  store::{ConnectionStore, ResourceStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

/// A published agent profile a step can run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileChoice {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub version: VersionNumber,
  /// The profile's model, which steps inherit unless they override it.
  pub model: ModelId,
  /// Name of the connection that runs the profile; `None` if it was removed.
  pub connection: Option<Arc<str>>,
  /// The CLI of that connection, which decides what a step can narrow the permissions to.
  pub provider: Option<AgentProvider>,
  /// The permissions steps narrow.
  pub permissions: AgentPermissions,
}

/// A published tool a step can call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolChoice {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub version: VersionNumber,
  pub kind: ToolKind,
  /// The command line of a command tool.
  pub command: Option<Arc<str>>,
  /// Whether a step can run it in this version: a command tool without access rules.
  pub runnable: bool,
}

/// What steps of the project's pipelines can be assigned to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assignables {
  pub profiles: Vec<ProfileChoice>,
  pub tools: Vec<ToolChoice>,
}

impl Assignables {
  pub fn load(store: &ProjectDb) -> Result<Self, StoreError> {
    let connections = store.connections()?;
    let mut profiles = Vec::new();
    for resource in store.resources(ResourceKind::AgentProfile)? {
      if let Some(version) = store.latest_version::<AgentProfile>(resource.id())? {
        let connection = connections
          .iter()
          .find(|connection| connection.id() == version.body().connection());
        let provider = connection.and_then(|connection| match connection.kind() {
          ConnectionKind::Agent(provider) => Some(*provider),
          ConnectionKind::GitHub { .. } => None,
        });
        profiles.push(ProfileChoice {
          connection: connection.map(|connection| connection.name().as_str().into()),
          provider,
          permissions: version.body().permissions().clone(),
          id: resource.id(),
          name: resource.name().as_str().into(),
          version: version.number(),
          model: version.body().model().clone(),
        });
      }
    }
    let mut tools = Vec::new();
    for resource in store.resources(ResourceKind::Tool)? {
      if let Some(version) = store.latest_version::<Tool>(resource.id())? {
        let tool = version.body();
        tools.push(ToolChoice {
          id: resource.id(),
          name: resource.name().as_str().into(),
          version: version.number(),
          kind: tool.kind(),
          command: match tool.action() {
            ToolAction::Command(command) => Some(command.command_line().into()),
            ToolAction::Http(_) | ToolAction::DatabaseQuery(_) => None,
          },
          runnable: tool.kind().runnable() && tool.access().is_none(),
        });
      }
    }
    Ok(Self { profiles, tools })
  }

  pub fn profile(&self, id: ResourceId) -> Option<&ProfileChoice> {
    self.profiles.iter().find(|profile| profile.id == id)
  }

  pub fn tool(&self, id: ResourceId) -> Option<&ToolChoice> {
    self.tools.iter().find(|tool| tool.id == id)
  }

  /// The published profile named `name` (ignoring case), if exactly one has that name.
  pub fn profile_named(&self, name: &str) -> Option<ResourceId> {
    single(self.profiles.iter().map(|profile| (profile.id, &*profile.name)), name)
  }

  /// The published tool named `name` (ignoring case), if exactly one has that name.
  pub fn tool_named(&self, name: &str) -> Option<ResourceId> {
    single(self.tools.iter().map(|tool| (tool.id, &*tool.name)), name)
  }
}

fn single<'a>(mut named: impl Iterator<Item = (ResourceId, &'a str)>, name: &str) -> Option<ResourceId> {
  let wanted = name.trim().to_lowercase();
  let found = named.find(|(_, candidate)| candidate.to_lowercase() == wanted)?;
  named
    .all(|(_, candidate)| candidate.to_lowercase() != wanted)
    .then_some(found.0)
}

/// The latest published version of a pipeline, as far as the editor shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
  pub number: VersionNumber,
  pub at: Timestamp,
  pub body: Pipeline,
}

/// A pipeline as the editor opens it: the draft if there is one, otherwise the latest
/// published version, which editing turns into a new draft.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenedPipeline {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub slug: Arc<str>,
  pub body: Pipeline,
  /// When the draft was last saved; `None` without a draft.
  pub draft_saved_at: Option<Timestamp>,
  /// The version the draft is based on, or the one a new draft would start from.
  pub based_on: Option<VersionNumber>,
  pub latest: Option<Published>,
  /// Tasks that use the pipeline.
  pub used_by: usize,
  pub assignables: Arc<Assignables>,
}

impl lurq::app::component::DevtoolsInspectable for OpenedPipeline {}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The pipeline was removed, or has neither a draft nor a version.
  #[error("the pipeline does not exist")]
  Missing,
}

impl OpenedPipeline {
  pub fn load(db: &ProjectDb, id: ResourceId) -> Result<Self, OpenError> {
    let resource = db.resource(id)?.ok_or(OpenError::Missing)?;
    let latest = db
      .latest_version::<Pipeline>(id)?
      .map(|version: Version<Pipeline>| Published {
        number: version.number(),
        at: version.published_at(),
        body: version.body().clone(),
      });
    let (body, draft_saved_at, based_on) = match db.draft::<Pipeline>(id)? {
      Some(draft) => (draft.body().clone(), Some(draft.updated_at()), draft.based_on()),
      None => {
        let version = latest.as_ref().ok_or(OpenError::Missing)?;
        (version.body.clone(), None, Some(version.number))
      }
    };
    Ok(Self {
      id,
      name: resource.name().as_str().into(),
      slug: resource.slug().as_str().into(),
      body,
      draft_saved_at,
      based_on,
      latest,
      used_by: tasks_using(db, id)?,
      assignables: Arc::new(Assignables::load(db)?),
    })
  }

  /// [`Self::load`] from the project database of `repo_root`.
  pub fn load_in(repo_root: &Path, id: ResourceId) -> Result<Self, OpenError> {
    Self::load(&ProjectDb::open(repo_root)?, id)
  }
}

/// Number of the project's tasks that use pipeline `id`.
pub fn tasks_using<S: TaskStore>(store: &S, id: ResourceId) -> Result<usize, S::Error> {
  Ok(
    store
      .tasks()?
      .iter()
      .filter(|task| task.details().pipeline == Some(id))
      .count(),
  )
}

/// Stores `draft` as the pipeline's draft, or removes the draft when it repeats its version.
pub fn save_draft_in(repo_root: &Path, id: ResourceId, draft: &Draft<Pipeline>) -> Result<DraftSaved, StoreError> {
  ProjectDb::open(repo_root)?.save_draft(id, draft)
}

/// Publishes the stored draft; the version it became.
pub fn publish_in(repo_root: &Path, id: ResourceId) -> Result<(VersionNumber, Timestamp), StoreError> {
  let version = ProjectDb::open(repo_root)?.publish_draft::<Pipeline>(id, Timestamp::now())?;
  Ok((version.number(), version.published_at()))
}
