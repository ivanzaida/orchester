//! Reading and saving agent profiles through the project database. Each profile follows the
//! shared lifecycle: Draft saved → Publish vN → Published vN (`docs/design/orchester-workspace.md`,
//! "Core workflow"). Every function blocks on SQLite; the UI calls them off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  connection::{AgentConnection, invocation::Platform, mcp::McpServer},
  id::ResourceId,
  permission::AgentPermissions,
  resource::{
    AgentProfile, Draft, ModelId, Pipeline, PublishError, ResourceKind, Slug, StepKind, Version, VersionNumber,
  },
  store::{ConnectionStore, McpServerStore, ResourceStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::connections::data::agent_connections;

/// Slug of a profile whose role has no ASCII letters or digits.
const FALLBACK_SLUG: &str = "agent";

/// Where a profile stands in the lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionState {
  /// A draft based on the latest version (or on none, before the first publish).
  Draft { published: Option<VersionNumber> },
  /// A draft started from an older version than the latest: publishing it is refused.
  StaleDraft {
    based_on: Option<VersionNumber>,
    published: Option<VersionNumber>,
  },
  /// No draft: the latest version is current.
  Published(VersionNumber),
}

impl VersionState {
  pub const fn published(self) -> Option<VersionNumber> {
    match self {
      Self::Draft { published } | Self::StaleDraft { published, .. } => published,
      Self::Published(version) => Some(version),
    }
  }

  pub const fn has_draft(self) -> bool {
    !matches!(self, Self::Published(_))
  }
}

/// A profile as the editor shows it: the draft if there is one, otherwise the latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileEntry {
  pub id: ResourceId,
  pub profile: AgentProfile,
  pub state: VersionState,
}

impl ProfileEntry {
  /// `None` for a resource with neither a draft nor a version, which the store never creates.
  pub fn new(
    id: ResourceId,
    draft: Option<Draft<AgentProfile>>,
    latest: Option<&Version<AgentProfile>>,
  ) -> Option<Self> {
    let published = latest.map(Version::number);
    let (profile, state) = match (draft, latest) {
      (Some(draft), _) => {
        let state = if draft.based_on() == published {
          VersionState::Draft { published }
        } else {
          VersionState::StaleDraft {
            based_on: draft.based_on(),
            published,
          }
        };
        (draft.body().clone(), state)
      }
      (None, Some(latest)) => (latest.body().clone(), VersionState::Published(latest.number())),
      (None, None) => return None,
    };
    Some(Self { id, profile, state })
  }
}

/// A published pipeline step that runs a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepUse {
  pub profile: ResourceId,
  pub pipeline: Arc<str>,
  pub step: Arc<str>,
  /// The step's own model instead of the profile's.
  pub model_override: Option<ModelId>,
}

/// Everything the agents screen shows, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectAgents {
  pub profiles: Arc<Vec<ProfileEntry>>,
  /// Persisted draft save times; independent of the editor’s optimistic content comparison.
  pub draft_times: Arc<std::collections::BTreeMap<ResourceId, Timestamp>>,
  pub usage: Arc<super::usage::Usage>,
  /// Latest published bodies, for comparing a draft with what new runs currently use.
  pub published: Arc<Vec<ProfileEntry>>,
  /// Agent provider connections: the only kind a profile can run on.
  pub connections: Arc<Vec<AgentConnection>>,
  pub steps: Arc<Vec<StepUse>>,
  /// The project's MCP servers, which a profile can give its agents.
  pub mcp: Arc<Vec<McpServer>>,
  /// Whether the project's agents may load the repository's own agent files (Project settings ›
  /// General), which the enforcement panel shows beside the profile's permissions.
  pub agent_files: RepositoryAgentFiles,
  /// The published rules and skills that reach profiles by their scope; read for the screen only.
  pub prompts: Arc<super::prompts::PromptSources>,
}

impl ProjectAgents {
  pub fn load<S>(store: &S) -> Result<Self, StoreError>
  where
    S: ResourceStore<Error = StoreError>
      + ConnectionStore<Error = StoreError>
      + McpServerStore<Error = StoreError>
      + SettingsStore<Error = StoreError>,
  {
    Ok(Self {
      profiles: Arc::new(profiles(store)?),
      draft_times: Arc::new(draft_times(store)?),
      usage: Arc::default(),
      published: Arc::new(published_profiles(store)?),
      connections: Arc::new(agent_connections(store.connections()?)),
      steps: Arc::new(step_uses(store)?),
      mcp: Arc::new(store.mcp_servers()?),
      agent_files: store.repository_agent_files()?,
      prompts: Arc::default(),
    })
  }

  pub fn load_in(repo_root: &Path) -> Result<Self, StoreError> {
    Self::load(&ProjectDb::open(repo_root)?)
  }

  /// Editor data plus pipeline uses, active runs and the rules and skills that reach profiles, for the screen.
  pub fn for_screen(repo_root: &Path) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let mut listed = Self::load(&db)?;
    listed.usage = Arc::new(super::usage::Usage::load(&db)?);
    listed.prompts = Arc::new(super::prompts::PromptSources::load(&db)?);
    Ok(listed)
  }

  pub fn profile(&self, id: ResourceId) -> Option<&ProfileEntry> {
    self.profiles.iter().find(|entry| entry.id == id)
  }
}

/// The project's agent profiles, ordered by slug.
pub fn profiles<S: ResourceStore>(store: &S) -> Result<Vec<ProfileEntry>, S::Error> {
  let mut entries = Vec::new();
  for resource in store.resources(ResourceKind::AgentProfile)? {
    let latest = store.latest_version(resource.id())?;
    match ProfileEntry::new(resource.id(), store.draft(resource.id())?, latest.as_ref()) {
      Some(entry) => entries.push(entry),
      // Nothing to show or edit; listing it would need content that does not exist.
      None => tracing::error!(resource = %resource.id(), "agent profile has neither a draft nor a version"),
    }
  }
  Ok(entries)
}

/// The project's agent profiles as their latest published versions, ordered by slug; a profile never published is
/// left out.
pub(crate) fn published_profiles<S: ResourceStore>(store: &S) -> Result<Vec<ProfileEntry>, S::Error> {
  let mut entries = Vec::new();
  for resource in store.resources(ResourceKind::AgentProfile)? {
    let latest = store.latest_version(resource.id())?;
    if let Some(entry) = ProfileEntry::new(resource.id(), None, latest.as_ref()) {
      entries.push(entry);
    }
  }
  Ok(entries)
}

/// Steps of the latest published pipeline versions that run a profile.
pub(crate) fn step_uses<S: ResourceStore>(store: &S) -> Result<Vec<StepUse>, S::Error> {
  let mut uses = Vec::new();
  for resource in store.resources(ResourceKind::Pipeline)? {
    let Some(version) = store.latest_version::<Pipeline>(resource.id())? else {
      continue;
    };
    for step in version.body().steps() {
      if let StepKind::Agent {
        profile: Some(profile),
        model,
        ..
      } = &step.kind
      {
        uses.push(StepUse {
          profile: *profile,
          pipeline: resource.name().as_str().into(),
          step: step.name.as_str().into(),
          model_override: model.clone(),
        });
      }
    }
  }
  Ok(uses)
}

/// Creates a profile whose first draft is `profile`, under a slug made from its role.
pub fn create_profile<S: ResourceStore>(
  store: &mut S,
  profile: &AgentProfile,
  now: Timestamp,
) -> Result<ResourceId, S::Error> {
  let taken: Vec<Slug> = store.slugs(ResourceKind::AgentProfile)?;
  let fallback = Slug::new(FALLBACK_SLUG).expect("the fallback slug is valid");
  let slug = Slug::unique_for(profile.role(), &fallback, &taken);
  let resource = store.create_resource(slug, profile.role().clone(), profile, now)?;
  Ok(resource.id())
}

/// Saves `profile` as the draft. An existing draft keeps the version it was started from; a
/// new draft starts from the latest published version.
pub fn save_draft<S: ResourceStore>(
  store: &mut S,
  id: ResourceId,
  profile: AgentProfile,
  now: Timestamp,
) -> Result<(), S::Error> {
  let based_on = match store.draft::<AgentProfile>(id)? {
    Some(draft) => draft.based_on(),
    None => store
      .latest_version::<AgentProfile>(id)?
      .map(|version| version.number()),
  };
  // The profile is read again after every change, which shows whether a draft remains.
  store.save_draft(id, &Draft::new(profile, based_on, now)).map(|_| ())
}

/// Saves an editor candidate only while the draft body, base and latest version still match.
pub fn save_draft_checked_in(root: &Path, expected: &ProfileEntry, profile: AgentProfile) -> Result<(), StoreError> {
  edit_checked_in(root, expected, move |draft, latest| {
    let based_on = draft.map_or_else(|| latest.map(Version::number), |draft| draft.based_on());
    Ok(Draft::new(profile, based_on, Timestamp::now()))
  })
}

fn edit_checked_in(
  root: &Path,
  expected: &ProfileEntry,
  edit: impl FnOnce(Option<Draft<AgentProfile>>, Option<&Version<AgentProfile>>) -> Result<Draft<AgentProfile>, StoreError>,
) -> Result<(), StoreError> {
  ProjectDb::open(root)?.update_draft(expected.id, |draft, latest| {
    if ProfileEntry::new(expected.id, draft.clone(), latest.as_ref()).as_ref() != Some(expected) {
      return Err(StoreError::DraftChanged(expected.id));
    }
    edit(draft, latest.as_ref())
  })?;
  Ok(())
}

/// Publishes the saved draft as the next version, if its connection's CLI can enforce its
/// permissions on this platform (`PublishError::Permissions`); a profile whose connection is gone
/// cannot say what is enforced (`PublishError::ConnectionMissing`).
pub fn publish<S>(store: &mut S, id: ResourceId, now: Timestamp) -> Result<VersionNumber, StoreError>
where
  S: ResourceStore<Error = StoreError> + ConnectionStore<Error = StoreError>,
{
  let draft = store.draft::<AgentProfile>(id)?.ok_or(StoreError::NoDraft(id))?;
  enforceable(store, draft.body())?;
  Ok(store.publish_draft::<AgentProfile>(id, now)?.number())
}

/// [`publish`], of the draft only if it is still `checked`, read in the transaction that publishes it: what
/// a caller checked before is what becomes the version ([`StoreError::DraftChanged`] otherwise).
pub fn publish_checked_in(
  repo_root: &Path,
  id: ResourceId,
  checked: &AgentProfile,
) -> Result<VersionNumber, StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  enforceable(&db, checked)?;
  Ok(db.publish_draft_matching(id, checked, Timestamp::now())?.number())
}

/// Publishes exactly the stale draft and latest version the person reviewed, atomically.
pub fn publish_stale_checked_in(root: &Path, expected: &ProfileEntry) -> Result<VersionNumber, StoreError> {
  let mut db = ProjectDb::open(root)?;
  enforceable(&db, &expected.profile)?;
  let now = Timestamp::now();
  db.publish_draft_with(expected.id, now, |draft: Draft<AgentProfile>, latest| {
    if ProfileEntry::new(expected.id, Some(draft.clone()), latest.as_ref()).as_ref() != Some(expected) {
      return Err(StoreError::DraftChanged(expected.id));
    }
    Ok(Draft::new(
      draft.body().clone(),
      latest.as_ref().map(Version::number),
      now,
    ))
  })
  .map(|version| version.number())
}

/// Refused when `profile`'s connection is gone, or its CLI cannot enforce the profile's permissions here.
fn enforceable<S: ConnectionStore<Error = StoreError>>(store: &S, profile: &AgentProfile) -> Result<(), StoreError> {
  let connection = agent_connections(store.connections()?)
    .into_iter()
    .find(|connection| connection.id() == profile.connection())
    .ok_or(PublishError::ConnectionMissing)?;
  profile
    .permissions()
    .enforced_on(connection.provider(), Platform::current())
    .map_err(PublishError::from)?;
  Ok(())
}

/// The user chose to publish a stale draft anyway: it is based on the latest version from now
/// on, so its content replaces what that version changed.
pub fn keep_draft_over_latest<S: ResourceStore>(store: &mut S, id: ResourceId, now: Timestamp) -> Result<(), S::Error> {
  let latest = store
    .latest_version::<AgentProfile>(id)?
    .map(|version| version.number());
  // Without a draft there is nothing to keep; reading the profile again shows its state.
  if let Some(draft) = store.draft::<AgentProfile>(id)? {
    store.save_draft(id, &Draft::new(draft.body().clone(), latest, now))?;
  }
  Ok(())
}

/// A change to one profile, applied in the project database.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileChange {
  Create(AgentProfile),
  SaveDraft(ResourceId, AgentProfile),
  Publish(ResourceId),
  KeepDraft(ResourceId),
}

/// Applies `change` in the project database of `repo_root` and returns the profile it touched.
pub fn apply_in(repo_root: &Path, change: ProfileChange) -> Result<ResourceId, StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  let now = Timestamp::now();
  match change {
    ProfileChange::Create(profile) => create_profile(&mut db, &profile, now),
    ProfileChange::SaveDraft(id, profile) => save_draft(&mut db, id, profile, now).map(|()| id),
    ProfileChange::Publish(id) => publish(&mut db, id, now).map(|_| id),
    ProfileChange::KeepDraft(id) => keep_draft_over_latest(&mut db, id, now).map(|()| id),
  }
}

/// Adds run permissions to profile `id`'s draft (started from the latest version when it has
/// none), as *Also save to the profile* in the run permissions popover asks; the draft keeps its
/// other changes. Returns the version the draft publishes as.
pub fn save_permissions_to_draft(
  repo_root: &Path,
  id: ResourceId,
  permissions: AgentPermissions,
) -> Result<u32, StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  let latest = db.latest_version::<AgentProfile>(id)?;
  let body = match db.draft::<AgentProfile>(id)? {
    Some(draft) => draft.body().clone(),
    None => latest.as_ref().ok_or(StoreError::ResourceNotFound(id))?.body().clone(),
  };
  save_draft(&mut db, id, body.with_permissions(permissions), Timestamp::now())?;
  Ok(latest.map_or(1, |latest| latest.number().get().saturating_add(1)))
}
fn draft_times<S: ResourceStore>(store: &S) -> Result<std::collections::BTreeMap<ResourceId, Timestamp>, S::Error> {
  let mut times = std::collections::BTreeMap::new();
  for resource in store.resources(ResourceKind::AgentProfile)? {
    if let Some(draft) = store.draft::<AgentProfile>(resource.id())? {
      times.insert(resource.id(), draft.updated_at());
    }
  }
  Ok(times)
}
