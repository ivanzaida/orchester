//! Reading, creating and publishing the project's skills through the project database, with the
//! roles and pipelines their scope can name, and adding a reference file from disk as a project
//! blob. Each skill follows the shared lifecycle, Draft saved → Publish vN → Published vN; drafts are
//! saved as they are edited (see [`crate::drafts`]). Every function blocks on SQLite or the disk;
//! the UI calls them off the UI thread.

use std::{collections::BTreeSet, fs, io, path::Path, sync::Arc};

use orchester_core::{
  agent_files::import::RepositoryOrigin,
  id::ResourceId,
  name::Name,
  resource::{Draft, InstructionScope, ResourceKind, Skill, SkillFile, SkillFilePath, Slug, Version, VersionNumber},
  store::{BlobStore, RepositoryImportStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use crate::{
  agents::data::VersionState,
  instructions::data::{PipelineChoice, ProfileChoice, pipelines, profiles},
};

/// Slug of a skill whose name has no ASCII letters or digits.
const FALLBACK_SLUG: &str = "skill";
/// How reference files are recorded among the project's blobs; what they hold is not checked.
const FILE_MEDIA_TYPE: &str = "application/octet-stream";

/// A skill as the library and the editor show it: the draft if there is one, otherwise the latest
/// version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillEntry {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub slug: Arc<str>,
  pub body: Skill,
  pub state: VersionState,
  pub last_published: Option<Timestamp>,
  /// Where it was imported from, if it was made from the repository.
  pub origin: Option<RepositoryOrigin>,
}

impl SkillEntry {
  /// `None` for a resource with neither a draft nor a version, which the store never creates.
  fn new(
    (id, name, slug): (ResourceId, Arc<str>, Arc<str>),
    draft: Option<Draft<Skill>>,
    latest: Option<&Version<Skill>>,
    origin: Option<RepositoryOrigin>,
  ) -> Option<Self> {
    let published = latest.map(Version::number);
    let (body, state) = match (draft, latest) {
      (Some(draft), _) if draft.based_on() == published => (draft.body().clone(), VersionState::Draft { published }),
      (Some(draft), _) => (
        draft.body().clone(),
        VersionState::StaleDraft {
          based_on: draft.based_on(),
          published,
        },
      ),
      (None, Some(latest)) => (latest.body().clone(), VersionState::Published(latest.number())),
      (None, None) => return None,
    };
    Some(Self {
      id,
      name,
      slug,
      body,
      state,
      last_published: latest.map(Version::published_at),
      origin,
    })
  }

  /// The version a draft of this skill starts from, or is based on.
  pub const fn based_on(&self) -> Option<VersionNumber> {
    match self.state {
      VersionState::Draft { published } => published,
      VersionState::StaleDraft { based_on, .. } => based_on,
      VersionState::Published(version) => Some(version),
    }
  }
}

/// Everything the skills screen shows and chooses from, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectSkills {
  pub entries: Arc<Vec<SkillEntry>>,
  pub profiles: Arc<Vec<ProfileChoice>>,
  pub pipelines: Arc<Vec<PipelineChoice>>,
}

impl lurq::app::component::DevtoolsInspectable for ProjectSkills {}

impl ProjectSkills {
  pub fn load(db: &ProjectDb) -> Result<Self, StoreError> {
    Ok(Self {
      entries: Arc::new(entries(db)?),
      profiles: Arc::new(profiles(db)?),
      pipelines: Arc::new(pipelines(db)?),
    })
  }

  pub fn load_in(repo_root: &Path) -> Result<Self, StoreError> {
    Self::load(&ProjectDb::open(repo_root)?)
  }

  pub fn entry(&self, id: ResourceId) -> Option<&SkillEntry> {
    self.entries.iter().find(|entry| entry.id == id)
  }

  /// The roles of the project's profiles, each once, in name order.
  pub fn roles(&self) -> BTreeSet<Name> {
    self.profiles.iter().map(|profile| profile.role.clone()).collect()
  }

  pub fn pipeline(&self, id: ResourceId) -> Option<&PipelineChoice> {
    self.pipelines.iter().find(|pipeline| pipeline.id == id)
  }
}

/// The project's skills, ordered by slug.
fn entries(db: &ProjectDb) -> Result<Vec<SkillEntry>, StoreError> {
  let mut entries = Vec::new();
  for resource in db.resources(ResourceKind::Skill)? {
    let id = resource.id();
    let latest = db.latest_version::<Skill>(id)?;
    let names = (id, resource.name().as_str().into(), resource.slug().as_str().into());
    match SkillEntry::new(names, db.draft(id)?, latest.as_ref(), db.repository_origin(id)?) {
      Some(entry) => entries.push(entry),
      // Nothing to show or edit; listing it would need content that does not exist.
      None => tracing::error!(resource = %id, "skill has neither a draft nor a version"),
    }
  }
  Ok(entries)
}

/// Creates a skill called `name` for the whole project as an empty draft, under a slug made from
/// its name.
pub fn create_in(repo_root: &Path, name: Name) -> Result<ResourceId, StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  let taken: Vec<Slug> = db
    .resources(ResourceKind::Skill)?
    .iter()
    .map(|resource| resource.slug().clone())
    .collect();
  let fallback = Slug::new(FALLBACK_SLUG).expect("the fallback slug is valid");
  let slug = Slug::unique_for(&name, &fallback, &taken);
  let body = Skill::new(String::new(), String::new(), Vec::new(), InstructionScope::Project)
    .expect("a skill without files has no two files at one path");
  Ok(db.create_resource(slug, name, &body, Timestamp::now())?.id())
}

/// Publishes the stored draft; the version it became.
pub fn publish_in(repo_root: &Path, id: ResourceId) -> Result<VersionNumber, StoreError> {
  Ok(
    ProjectDb::open(repo_root)?
      .publish_draft::<Skill>(id, Timestamp::now())?
      .number(),
  )
}

#[derive(Debug, Error)]
pub enum AddFileError {
  #[error("cannot read {0}: {1}")]
  Read(String, io::Error),
  /// The file's name cannot be a reference file's path.
  #[error("`{0}` cannot name a reference file")]
  Name(String),
  #[error(transparent)]
  Store(#[from] StoreError),
}

/// The file at `path` as a reference file at the skill folder's top, its content stored among the
/// project's blobs. The skill's draft takes it when the editor saves.
pub fn add_file_in(repo_root: &Path, path: &Path) -> Result<SkillFile, AddFileError> {
  let name = path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_default();
  let file_path = SkillFilePath::new(&name).map_err(|_| AddFileError::Name(name.clone()))?;
  let content = fs::read(path).map_err(|error| AddFileError::Read(name, error))?;
  let hash = ProjectDb::open(repo_root)?.put_blob(FILE_MEDIA_TYPE, &content)?;
  Ok(SkillFile {
    path: file_path,
    content: hash,
    size: content.len() as u64,
  })
}

/// Stores `content` among the project's blobs for a reference file.
pub(crate) fn put_file(db: &mut ProjectDb, content: &[u8]) -> Result<orchester_core::blob::ContentHash, StoreError> {
  db.put_blob(FILE_MEDIA_TYPE, content)
}
