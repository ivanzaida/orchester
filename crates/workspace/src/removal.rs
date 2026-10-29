//! Discarding a draft and removing an agent profile, instruction, skill or tool (ORC-107), as the Agents hub and the
//! domain tools ask for them. Every function blocks on SQLite; the UI calls them off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    AgentProfile, InstructionScope, Pipeline, Removal, RemoveMode, ResourceBody, ResourceKind, Skill, StepUser,
    VersionNumber, step_users,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  messages::{self, Message},
  tasks::outside::Outside,
};

/// Who a resource reaches, which its removal would take away from them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Users {
  /// Pipeline steps run it (a profile or a tool).
  Steps(Vec<StepUser>),
  /// The agents a skill's scope reaches.
  Reach(Vec<Reached>),
}

impl Users {
  pub fn is_empty(&self) -> bool {
    match self {
      Self::Steps(steps) => steps.is_empty(),
      Self::Reach(reached) => reached.is_empty(),
    }
  }
}

/// An agent profile, pipeline or step a skill reaches through its scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reached {
  /// A profile, through the project scope or its role.
  Profile {
    name: Arc<str>,
    by_role: bool,
  },
  Pipeline(Arc<str>),
  Step {
    pipeline: Arc<str>,
    step: Arc<str>,
  },
}

/// Who resource `id` of `kind` reaches now: what the in-place question of Remove lists. Instructions reach agents by
/// their scope but nobody is reset by their removal, so they list no one.
pub fn users_in(root: &Path, id: ResourceId, kind: ResourceKind) -> Result<Users, Message> {
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let users = match kind {
    ResourceKind::AgentProfile | ResourceKind::Tool => step_users(&db, id).map(Users::Steps),
    ResourceKind::Skill => skill_reach(&db, id).map(Users::Reach),
    // No step runs a test suite yet.
    ResourceKind::Instruction | ResourceKind::Pipeline | ResourceKind::TestSuite => Ok(Users::Steps(Vec::new())),
  };
  users.map_err(|error| messages::store_error(&error))
}

/// Removes resource `id`; with [`RemoveMode::Reset`] the steps that ran it are cleared first. What the screens that
/// show the change read again is [`changed_by_removal`].
pub fn remove_in(root: &Path, id: ResourceId, mode: RemoveMode) -> Result<Removal, Message> {
  ProjectDb::open(root)
    .and_then(|mut db| db.remove_resource(id, mode, Timestamp::now()))
    .map_err(|error| messages::store_error(&error))
}

/// What removing a resource of `kind` changed besides its own list: a profile's or tool's removal resets pipeline
/// drafts. No screen lists test suites yet.
pub const fn changed_by_removal(kind: ResourceKind) -> &'static [Outside] {
  match kind {
    ResourceKind::AgentProfile => &[Outside::Profiles, Outside::Pipelines],
    ResourceKind::Tool => &[Outside::Tools, Outside::Pipelines],
    ResourceKind::Instruction => &[Outside::Instructions],
    ResourceKind::Skill => &[Outside::Skills],
    ResourceKind::Pipeline => &[Outside::Pipelines],
    ResourceKind::TestSuite => &[],
  }
}

/// Discards the draft of resource `id` while it is still `expected`: the latest published version is the resource
/// again, and that version's number comes back.
pub fn discard_in<B: ResourceBody>(root: &Path, id: ResourceId, expected: &B) -> Result<VersionNumber, Message> {
  ProjectDb::open(root)
    .and_then(|mut db| db.discard_draft_matching(id, expected))
    .map_err(|error| messages::store_error(&error))
}

/// The profiles, pipelines and steps skill `id`'s draft or latest version reaches.
fn skill_reach(db: &ProjectDb, id: ResourceId) -> Result<Vec<Reached>, StoreError> {
  let skill = match db.draft::<Skill>(id)? {
    Some(draft) => draft.body().clone(),
    None => match db.latest_version::<Skill>(id)? {
      Some(version) => version.body().clone(),
      None => return Ok(Vec::new()),
    },
  };
  match skill.scope() {
    InstructionScope::Project => profiles(db, None),
    InstructionScope::AgentRole { role } => profiles(db, Some(role)),
    InstructionScope::Pipeline { pipeline } => Ok(
      pipeline_name(db, *pipeline)?
        .map(|name| vec![Reached::Pipeline(name)])
        .unwrap_or_default(),
    ),
    InstructionScope::Step { pipeline, step } => {
      let Some(name) = pipeline_name(db, *pipeline)? else {
        return Ok(Vec::new());
      };
      let body = match db.draft::<Pipeline>(*pipeline)? {
        Some(draft) => Some(draft.body().clone()),
        None => db
          .latest_version::<Pipeline>(*pipeline)?
          .map(|version| version.body().clone()),
      };
      Ok(
        body
          .and_then(|body| body.step(step).map(|step| Arc::from(step.name.as_str())))
          .map(|step| vec![Reached::Step { pipeline: name, step }])
          .unwrap_or_default(),
      )
    }
  }
}

/// The project's profiles, or those whose role is `role`, by the role they show.
fn profiles(db: &ProjectDb, role: Option<&Name>) -> Result<Vec<Reached>, StoreError> {
  let mut reached = Vec::new();
  for entry in crate::agents::data::profiles(db)? {
    let profile: &AgentProfile = &entry.profile;
    if role.is_none_or(|role| profile.role() == role) {
      reached.push(Reached::Profile {
        name: profile.role().as_str().into(),
        by_role: role.is_some(),
      });
    }
  }
  Ok(reached)
}

fn pipeline_name(db: &ProjectDb, id: ResourceId) -> Result<Option<Arc<str>>, StoreError> {
  Ok(
    db.resources(ResourceKind::Pipeline)?
      .into_iter()
      .find(|resource| resource.id() == id)
      .map(|resource| resource.name().as_str().into()),
  )
}
