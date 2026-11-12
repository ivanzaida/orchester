//! What the Pipelines hub of a local project lists (`Pipelines · v2` `pCzIS`, P1): each pipeline with its steps, the
//! tasks that use it, its version and whether it can run as it is, and what was archived. A pipeline needs the person
//! when a profile or tool its steps run is not ready on this computer
//! ([`crate::readiness`]).

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind, VersionNumber},
  secret::SecretVault,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{
  data::Assignables,
  flow::{FlowRead, Stored},
  library::{LibraryEntry, library},
  ready::{UndecidedMember, undecided_members},
};
use crate::{
  messages::{self, Message},
  readiness::{Group, ReadinessError, ReadinessStanding},
  workspace::Workspace,
};

/// Why the hub could not be read.
#[derive(Debug, Error)]
pub enum HubError {
  #[error(transparent)]
  Readiness(#[from] ReadinessError),
  #[error(transparent)]
  Store(#[from] StoreError),
  /// A pipeline the library listed has no version any more (another process removed it while the hub was read), so
  /// its steps cannot be drawn. The read fails rather than list a pipeline without them.
  #[error("pipeline {0} has no version to draw")]
  Unreadable(ResourceId),
}

impl HubError {
  /// What to tell the person.
  pub fn message(&self) -> Message {
    match self {
      Self::Readiness(error) => error.message(),
      Self::Store(error) => messages::store_error(error),
      Self::Unreadable(_) => Message::new("pipelines.flow.unreadable"),
    }
  }
}

/// One pipeline of the project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HubPipeline {
  pub id: ResourceId,
  pub name: Arc<str>,
  /// How tools, Orchester's MCP server and shared tasks name it: made from the name when it was created, never
  /// changed. A new pipeline's name may not give it again.
  pub slug: Arc<str>,
  pub step_count: usize,
  /// Its latest version.
  pub version: VersionNumber,
  /// The tasks that use it.
  pub used_by: usize,
  /// When its latest version was published.
  pub published_at: Timestamp,
  /// How the profiles and tools its latest version runs stand on this computer; `None` when the readiness does not
  /// list it.
  pub standing: Option<ReadinessStanding>,
  /// Every profile or tool its latest version runs that cannot be told to be ready; the standing names only the first.
  pub undecided: Vec<UndecidedMember>,
  /// Its latest version's steps as the panel draws them.
  pub flow: Arc<FlowRead>,
  /// The steps the flow was read from, which an edit of the page starts from.
  pub body: Arc<Pipeline>,
}

impl HubPipeline {
  /// [`Group::NeedsYouHere`] when it cannot run as it is: what its steps run is not ready;
  /// [`Group::Ready`] otherwise.
  pub fn group(&self) -> Group {
    match &self.standing {
      Some(standing) if standing.is_ready() => Group::Ready,
      _ => Group::NeedsYouHere,
    }
  }
}

/// A pipeline that was archived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchivedPipeline {
  pub id: ResourceId,
  pub name: Arc<str>,
  /// What it is stored by: an archived pipeline keeps its slug, which a new pipeline's name may not give again.
  pub slug: Arc<str>,
  pub at: Timestamp,
  /// Its latest version and when it was published; `None` when it never had one.
  pub latest: Option<(VersionNumber, Timestamp)>,
}

/// The pipelines of a local project, and the archived ones.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PipelineHub {
  pipelines: Vec<HubPipeline>,
  archived: Vec<ArchivedPipeline>,
  assignables: Arc<Assignables>,
}

impl PipelineHub {
  /// The pipelines by slug.
  pub fn pipelines(&self) -> &[HubPipeline] {
    &self.pipelines
  }

  /// What was archived, the latest first.
  pub fn archived(&self) -> &[ArchivedPipeline] {
    &self.archived
  }

  /// What the steps of the pipelines can run, which the flow of an edit is drawn with.
  pub fn assignables(&self) -> Arc<Assignables> {
    Arc::clone(&self.assignables)
  }
}

impl Workspace {
  /// The pipelines hub of the local project at `repo_root`. Reads the database and the keychain's list of what it has,
  /// so it blocks: run it on the background pool, on an explicit change and never on a timer
  /// ([`Workspace::project_readiness`]).
  pub fn pipeline_hub(&self, repo_root: &Path, vault: &dyn SecretVault) -> Result<PipelineHub, HubError> {
    let db = ProjectDb::open(repo_root)?;
    let readiness = self.readiness_in(&db, repo_root, vault)?;
    let mut assignables = Assignables::load(&db)?;
    assignables.standings = readiness
      .resources()
      .iter()
      .map(|found| (found.resource(), found.standing().clone()))
      .collect();
    let mut pipelines = Vec::new();
    for entry in library(&db)? {
      let standing = readiness.pipeline(entry.id).map(|found| found.standing().clone());
      let undecided = match &standing {
        Some(ReadinessStanding::Undecided(_)) => undecided_members(&db, entry.id, &readiness)?,
        _ => Vec::new(),
      };
      let stored = FlowRead::of(&db, entry.id, &assignables)?.ok_or(HubError::Unreadable(entry.id))?;
      pipelines.push(of_entry(entry, (standing, undecided), stored));
    }
    Ok(PipelineHub {
      pipelines,
      archived: archived(&db)?,
      assignables: Arc::new(assignables),
    })
  }
}

fn of_entry(
  entry: LibraryEntry,
  (standing, undecided): (Option<ReadinessStanding>, Vec<UndecidedMember>),
  stored: Stored,
) -> HubPipeline {
  HubPipeline {
    id: entry.id,
    name: entry.name,
    slug: entry.slug,
    step_count: entry.step_count,
    version: entry.version,
    used_by: entry.used_by,
    published_at: entry.last_published,
    standing,
    undecided,
    flow: Arc::new(stored.read),
    body: Arc::new(stored.body),
  }
}

fn archived(db: &ProjectDb) -> Result<Vec<ArchivedPipeline>, StoreError> {
  let mut archived = Vec::new();
  for (resource, at) in db.archived_local_resources(ResourceKind::Pipeline)? {
    let latest = db
      .latest_version::<Pipeline>(resource.id())?
      .map(|version| (version.number(), version.published_at()));
    archived.push(ArchivedPipeline {
      id: resource.id(),
      name: resource.name().as_str().into(),
      slug: resource.slug().as_str().into(),
      at,
      latest,
    });
  }
  archived.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.name.cmp(&b.name)));
  Ok(archived)
}
