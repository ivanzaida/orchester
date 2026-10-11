//! Reading and saving project tools through the project database (06.01 `ZFwWl`, 06.02 `YEWFo`).
//! Each tool follows the shared lifecycle: Draft saved → Publish vN → Published vN; runs pin the
//! version they start with. Only command tools can be created in this version. Every function
//! blocks on SQLite; the UI calls them off the UI thread.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, Pipeline, ResourceKind, Slug, StepKind, Tool, Version, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::agents::data::VersionState;

/// Slug of a tool whose name has no ASCII letters or digits.
const FALLBACK_SLUG: &str = "tool";

/// A tool as the library and the editor show it: the draft if there is one, otherwise the latest
/// version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolEntry {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub tool: Tool,
  pub state: VersionState,
}

impl ToolEntry {
  /// Whether this kind of tool runs today, without an unimplemented secret policy.
  pub fn runnable(&self) -> bool {
    self.tool.kind().runnable() && self.tool.access().is_none()
  }
  /// `None` for a resource with neither a draft nor a version, which the store never creates.
  pub fn new(id: ResourceId, name: &Name, draft: Option<Draft<Tool>>, latest: Option<&Version<Tool>>) -> Option<Self> {
    let published = latest.map(Version::number);
    let (tool, state) = match (draft, latest) {
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
    Some(Self {
      id,
      name: name.as_str().into(),
      tool,
      state,
    })
  }
}

/// A step of a pipeline's latest published version that calls a tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolUse {
  pub tool: ResourceId,
  pub pipeline: ResourceId,
  pub pipeline_name: Arc<str>,
  pub version: VersionNumber,
  pub step: Arc<str>,
  /// One-based position in the published pipeline.
  pub ordinal: usize,
}

/// A pipeline whose latest published version calls tools, for "Used in your pipelines".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineTools {
  pub pipeline: ResourceId,
  pub name: Arc<str>,
  pub version: VersionNumber,
  /// Distinct tools its steps call.
  pub tools: usize,
}

/// Everything the tools screens show, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectTools {
  pub tools: Arc<Vec<ToolEntry>>,
  pub uses: Arc<Vec<ToolUse>>,
}

impl ProjectTools {
  pub fn load<S: ResourceStore>(store: &S) -> Result<Self, S::Error> {
    Ok(Self {
      tools: Arc::new(tools(store)?),
      uses: Arc::new(tool_uses(store)?),
    })
  }

  pub fn load_in(repo_root: &Path) -> Result<Self, StoreError> {
    Self::load(&ProjectDb::open(repo_root)?)
  }

  pub fn tool(&self, id: ResourceId) -> Option<&ToolEntry> {
    self.tools.iter().find(|entry| entry.id == id)
  }

  /// The published pipeline steps that call `tool`.
  pub fn used_by(&self, tool: ResourceId) -> impl Iterator<Item = &ToolUse> {
    self.uses.iter().filter(move |used| used.tool == tool)
  }

  /// Pipelines that call tools, in the order the store lists them.
  pub fn pipelines(&self) -> Vec<PipelineTools> {
    let mut pipelines: Vec<PipelineTools> = Vec::new();
    let mut tools: BTreeMap<ResourceId, Vec<ResourceId>> = BTreeMap::new();
    for used in self.uses.iter() {
      let called = tools.entry(used.pipeline).or_default();
      if !called.contains(&used.tool) {
        called.push(used.tool);
      }
      if !pipelines.iter().any(|pipeline| pipeline.pipeline == used.pipeline) {
        pipelines.push(PipelineTools {
          pipeline: used.pipeline,
          name: used.pipeline_name.clone(),
          version: used.version,
          tools: 0,
        });
      }
    }
    for pipeline in &mut pipelines {
      pipeline.tools = tools.get(&pipeline.pipeline).map_or(0, Vec::len);
    }
    pipelines
  }
}

/// The project's tools, ordered by slug.
pub fn tools<S: ResourceStore>(store: &S) -> Result<Vec<ToolEntry>, S::Error> {
  let mut entries = Vec::new();
  for resource in store.resources(ResourceKind::Tool)? {
    let latest = store.latest_version(resource.id())?;
    match ToolEntry::new(
      resource.id(),
      resource.name(),
      store.draft(resource.id())?,
      latest.as_ref(),
    ) {
      Some(entry) => entries.push(entry),
      // Nothing to show or edit; listing it would need content that does not exist.
      None => tracing::error!(resource = %resource.id(), "tool has neither a draft nor a version"),
    }
  }
  Ok(entries)
}

/// Steps of the latest published pipeline versions that call a tool.
fn tool_uses<S: ResourceStore>(store: &S) -> Result<Vec<ToolUse>, S::Error> {
  let mut uses = Vec::new();
  for resource in store.resources(ResourceKind::Pipeline)? {
    let Some(version) = store.latest_version::<Pipeline>(resource.id())? else {
      continue;
    };
    for (index, step) in version.body().steps().iter().enumerate() {
      if let StepKind::Tool { tool: Some(tool) } = &step.kind {
        uses.push(ToolUse {
          tool: *tool,
          pipeline: resource.id(),
          pipeline_name: resource.name().as_str().into(),
          version: version.number(),
          step: step.name.as_str().into(),
          ordinal: index + 1,
        });
      }
    }
  }
  Ok(uses)
}

/// Creates a tool called `name` whose first draft is `tool`, under a slug made from its name.
pub fn create_tool<S: ResourceStore>(
  store: &mut S,
  name: &Name,
  tool: &Tool,
  now: Timestamp,
) -> Result<ResourceId, S::Error> {
  let taken: Vec<Slug> = store
    .resources(ResourceKind::Tool)?
    .iter()
    .map(|resource| resource.slug().clone())
    .collect();
  let fallback = Slug::new(FALLBACK_SLUG).expect("the fallback slug is valid");
  let slug = Slug::unique_for(name, &fallback, &taken);
  Ok(store.create_resource(slug, name.clone(), tool, now)?.id())
}

/// Saves `tool` as the draft. An existing draft keeps the version it was started from; a new draft
/// starts from the latest published version.
pub fn save_draft<S: ResourceStore>(store: &mut S, id: ResourceId, tool: Tool, now: Timestamp) -> Result<(), S::Error> {
  let based_on = match store.draft::<Tool>(id)? {
    Some(draft) => draft.based_on(),
    None => store.latest_version::<Tool>(id)?.map(|version| version.number()),
  };
  // The tool is read again after every change, which shows whether a draft remains.
  store.save_draft(id, &Draft::new(tool, based_on, now)).map(|_| ())
}

/// The user chose to publish a stale draft anyway: it is based on the latest version from now
/// on, so its content replaces what that version changed.
pub fn keep_draft_over_latest<S: ResourceStore>(store: &mut S, id: ResourceId, now: Timestamp) -> Result<(), S::Error> {
  let latest = store.latest_version::<Tool>(id)?.map(|version| version.number());
  // Without a draft there is nothing to keep; reading the tool again shows its state.
  if let Some(draft) = store.draft::<Tool>(id)? {
    store.save_draft(id, &Draft::new(draft.body().clone(), latest, now))?;
  }
  Ok(())
}

/// A change to one tool, applied in the project database.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolChange {
  Create(Name, Tool),
  SaveDraft(ResourceId, Tool),
  Publish(ResourceId),
  KeepDraft(ResourceId),
}

/// Applies `change` in the project database of `repo_root` and returns the tool it touched.
pub fn apply_in(repo_root: &Path, change: ToolChange) -> Result<ResourceId, StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  let now = Timestamp::now();
  match change {
    ToolChange::Create(name, tool) => create_tool(&mut db, &name, &tool, now),
    ToolChange::SaveDraft(id, tool) => save_draft(&mut db, id, tool, now).map(|()| id),
    ToolChange::Publish(id) => db.publish_draft::<Tool>(id, now).map(|_| id),
    ToolChange::KeepDraft(id) => keep_draft_over_latest(&mut db, id, now).map(|()| id),
  }
}
