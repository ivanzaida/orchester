//! Reading, creating and publishing the project's instructions through the project database, with
//! the profiles and pipelines they can apply to. Each instruction follows the shared lifecycle:
//! Draft saved → Publish vN → Published vN (`docs/design/orchester-workspace.md`, "Core workflow");
//! drafts are saved as they are edited (see [`crate::drafts`]). Every function blocks on SQLite; the
//! UI calls them off the UI thread.

use std::{collections::BTreeSet, path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    AgentProfile, AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, Pipeline, PromptText,
    ResourceKind, Slug, StepKey, Version, VersionNumber,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::agents::data::VersionState;

/// Slug of an instruction whose name has no ASCII letters or digits.
const FALLBACK_SLUG: &str = "instruction";

/// An instruction as the editor shows it: the draft if there is one, otherwise the latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionEntry {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub slug: Arc<str>,
  pub body: Instruction,
  pub state: VersionState,
  /// When the draft was last saved; `None` without a draft.
  pub draft_saved_at: Option<Timestamp>,
}

impl InstructionEntry {
  /// `None` for a resource with neither a draft nor a version, which the store never creates.
  fn new(
    id: ResourceId,
    (name, slug): (Arc<str>, Arc<str>),
    draft: Option<Draft<Instruction>>,
    latest: Option<&Version<Instruction>>,
  ) -> Option<Self> {
    let published = latest.map(Version::number);
    let (body, state, draft_saved_at) = match (draft, latest) {
      (Some(draft), _) => {
        let state = if draft.based_on() == published {
          VersionState::Draft { published }
        } else {
          VersionState::StaleDraft {
            based_on: draft.based_on(),
            published,
          }
        };
        (draft.body().clone(), state, Some(draft.updated_at()))
      }
      (None, Some(latest)) => (latest.body().clone(), VersionState::Published(latest.number()), None),
      (None, None) => return None,
    };
    Some(Self {
      id,
      name,
      slug,
      body,
      state,
      draft_saved_at,
    })
  }

  /// The version a draft of this instruction starts from, or is based on.
  pub const fn based_on(&self) -> Option<VersionNumber> {
    match self.state {
      VersionState::Draft { published } => published,
      VersionState::StaleDraft { based_on, .. } => based_on,
      VersionState::Published(version) => Some(version),
    }
  }
}

/// An agent profile an instruction can select, or apply to by its role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileChoice {
  pub id: ResourceId,
  pub role: Name,
  pub slug: Arc<str>,
}

/// A pipeline step an instruction can apply to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepChoice {
  pub key: StepKey,
  pub name: Arc<str>,
}

/// A pipeline and its steps, as its draft or latest version has them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineChoice {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub steps: Vec<StepChoice>,
}

/// Everything the instructions screen edits and chooses from, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectInstructions {
  pub entries: Arc<Vec<InstructionEntry>>,
  pub profiles: Arc<Vec<ProfileChoice>>,
  pub pipelines: Arc<Vec<PipelineChoice>>,
}

impl ProjectInstructions {
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

  pub fn entry(&self, id: ResourceId) -> Option<&InstructionEntry> {
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

/// The project's instructions, ordered by slug.
fn entries(db: &ProjectDb) -> Result<Vec<InstructionEntry>, StoreError> {
  let mut entries = Vec::new();
  for resource in db.resources(ResourceKind::Instruction)? {
    let id = resource.id();
    let latest = db.latest_version::<Instruction>(id)?;
    let names = (resource.name().as_str().into(), resource.slug().as_str().into());
    match InstructionEntry::new(id, names, db.draft(id)?, latest.as_ref()) {
      Some(entry) => entries.push(entry),
      // Nothing to show or edit; listing it would need content that does not exist.
      None => tracing::error!(resource = %id, "instruction has neither a draft nor a version"),
    }
  }
  Ok(entries)
}

/// The project's profiles with the role of their draft, or else of their latest version.
pub(crate) fn profiles(db: &ProjectDb) -> Result<Vec<ProfileChoice>, StoreError> {
  let mut profiles = Vec::new();
  for resource in db.resources(ResourceKind::AgentProfile)? {
    let id = resource.id();
    let body = match db.draft::<AgentProfile>(id)? {
      Some(draft) => Some(draft.body().clone()),
      None => db
        .latest_version::<AgentProfile>(id)?
        .map(|version| version.body().clone()),
    };
    if let Some(body) = body {
      profiles.push(ProfileChoice {
        id,
        role: body.role().clone(),
        slug: resource.slug().as_str().into(),
      });
    }
  }
  Ok(profiles)
}

/// The project's pipelines with the steps of their draft, or else of their latest version.
pub(crate) fn pipelines(db: &ProjectDb) -> Result<Vec<PipelineChoice>, StoreError> {
  let mut pipelines = Vec::new();
  for resource in db.resources(ResourceKind::Pipeline)? {
    let id = resource.id();
    let body = match db.draft::<Pipeline>(id)? {
      Some(draft) => Some(draft.body().clone()),
      None => db.latest_version::<Pipeline>(id)?.map(|version| version.body().clone()),
    };
    if let Some(body) = body {
      pipelines.push(PipelineChoice {
        id,
        name: resource.name().as_str().into(),
        steps: body
          .steps()
          .iter()
          .map(|step| StepChoice {
            key: step.key.clone(),
            name: step.name.as_str().into(),
          })
          .collect(),
      });
    }
  }
  Ok(pipelines)
}

/// A new instruction: its name and where it applies. It starts as an empty draft for every agent in
/// its scope, injected when an agent starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewInstruction {
  pub name: Name,
  pub scope: InstructionScope,
}

/// Creates the instruction as an empty draft under a slug made from its name.
pub fn create(db: &mut ProjectDb, new: NewInstruction, now: Timestamp) -> Result<ResourceId, StoreError> {
  let taken: Vec<Slug> = db
    .resources(ResourceKind::Instruction)?
    .iter()
    .map(|resource| resource.slug().clone())
    .collect();
  let fallback = Slug::new(FALLBACK_SLUG).expect("the fallback slug is valid");
  let slug = Slug::unique_for(&new.name, &fallback, &taken);
  let body = Instruction::new(
    PromptText::new("").expect("empty text has no placeholders"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    new.scope,
  )
  .expect("every agent at start is a valid selection in any scope");
  Ok(db.create_resource(slug, new.name, &body, now)?.id())
}

pub fn create_in(repo_root: &Path, new: NewInstruction) -> Result<ResourceId, StoreError> {
  create(&mut ProjectDb::open(repo_root)?, new, Timestamp::now())
}

/// Publishes the stored draft; the version it became.
pub fn publish_in(repo_root: &Path, id: ResourceId) -> Result<(VersionNumber, Timestamp), StoreError> {
  let version = ProjectDb::open(repo_root)?.publish_draft::<Instruction>(id, Timestamp::now())?;
  Ok((version.number(), version.published_at()))
}
