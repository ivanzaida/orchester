//! What the Pipelines hub of a local project lists (`Pipelines · v2` `pCzIS`, P1): each pipeline with its steps, the
//! tasks that use it, its version and whether it can run as it is, and what was archived. A pipeline needs the person
//! when it has no published version, or when a profile or tool its steps run is not ready on this computer
//! ([`crate::readiness`]).

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, ResourceKind, VersionNumber},
  secret::SecretVault,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{
  data::Assignables,
  flow::FlowRead,
  library::{LibraryEntry, VersionState, library},
  problems::problem_view,
};
use crate::{
  messages::{self, Message},
  readiness::{Group, ReadinessError, ReadinessStanding},
  workspace::Workspace,
};

/// Joins the steps of a cycle in a problem's words.
const CYCLE_ARROW: &str = "→";

/// Why the hub could not be read.
#[derive(Debug, Error)]
pub enum HubError {
  #[error(transparent)]
  Readiness(#[from] ReadinessError),
  #[error(transparent)]
  Store(#[from] StoreError),
  /// A pipeline the library listed has neither a draft nor a version any more (another process removed it while the
  /// hub was read), so its steps cannot be drawn. The read fails rather than list a pipeline without them.
  #[error("pipeline {0} has no draft or version to draw")]
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
  pub step_count: usize,
  pub state: VersionState,
  /// The tasks that use it.
  pub used_by: usize,
  /// When its latest version was published; `None` while it has only a draft.
  pub published_at: Option<Timestamp>,
  /// How the profiles and tools its latest version runs stand on this computer; `None` while it has no version.
  pub standing: Option<ReadinessStanding>,
  /// What keeps a pipeline without a version from being published, the first problem of its draft.
  pub problem: Option<Message>,
  /// Its steps as the panel draws them: the draft's, or the latest version's.
  pub flow: Arc<FlowRead>,
}

impl HubPipeline {
  /// The latest published version; `None` while it has only a draft.
  pub const fn number(&self) -> Option<VersionNumber> {
    match self.state {
      VersionState::DraftOnly => None,
      VersionState::Published(number) | VersionState::PublishedWithDraft(number) => Some(number),
    }
  }

  /// Whether it was never published.
  pub const fn is_draft(&self) -> bool {
    matches!(self.state, VersionState::DraftOnly)
  }

  /// [`Group::NeedsYouHere`] when it cannot run as it is: it has no version, or what its steps run is not ready;
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
  pub at: Timestamp,
  /// Its latest version and when it was published; `None` when it never had one.
  pub latest: Option<(VersionNumber, Timestamp)>,
}

/// The pipelines of a local project, and the archived ones.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PipelineHub {
  pipelines: Vec<HubPipeline>,
  archived: Vec<ArchivedPipeline>,
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
}

impl Workspace {
  /// The pipelines hub of the local project at `repo_root`. Reads the database and the keychain's list of what it has,
  /// so it blocks: run it on the background pool, on an explicit change and never on a timer
  /// ([`Workspace::project_readiness`]).
  pub fn pipeline_hub(&self, repo_root: &Path, vault: &dyn SecretVault) -> Result<PipelineHub, HubError> {
    let db = ProjectDb::open(repo_root)?;
    let readiness = self.readiness_in(&db, repo_root, vault)?;
    let assignables = Assignables::load(&db)?;
    let mut pipelines = Vec::new();
    for entry in library(&db)? {
      let standing = readiness.pipeline(entry.id).map(|found| found.standing().clone());
      let problem = match entry.state {
        VersionState::DraftOnly => draft_problem(&db, entry.id)?,
        _ => None,
      };
      let flow = FlowRead::of(&db, entry.id, &assignables)?.ok_or(HubError::Unreadable(entry.id))?;
      pipelines.push(of_entry(entry, (standing, problem), Arc::new(flow)));
    }
    Ok(PipelineHub {
      pipelines,
      archived: archived(&db)?,
    })
  }
}

fn of_entry(
  entry: LibraryEntry,
  (standing, problem): (Option<ReadinessStanding>, Option<Message>),
  flow: Arc<FlowRead>,
) -> HubPipeline {
  HubPipeline {
    id: entry.id,
    name: entry.name,
    step_count: entry.step_count,
    state: entry.state,
    used_by: entry.used_by,
    published_at: entry.last_published,
    standing,
    problem,
    flow,
  }
}

/// The first thing that keeps the draft of `pipeline` from being published, in the editor's words.
fn draft_problem(db: &ProjectDb, pipeline: ResourceId) -> Result<Option<Message>, StoreError> {
  let Some(draft) = db.draft::<Pipeline>(pipeline)? else {
    return Ok(None);
  };
  let edit = PipelineEdit::new(draft.body().clone());
  Ok(
    edit
      .problems()
      .first()
      .map(|problem| problem_view(&edit, problem, CYCLE_ARROW).message),
  )
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
      at,
      latest,
    });
  }
  archived.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.name.cmp(&b.name)));
  Ok(archived)
}
