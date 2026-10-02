//! Who runs on which model of an agent connection, for the rule that a listed model in use stays listed
//! (core's [`ensure_used_kept`]): the latest published version and the draft of every profile, each on
//! its own connection and model; the profile versions that unfinished runs pinned; and every step of a
//! pipeline's latest published version or draft whose own model overrides its profile's, on each
//! connection that profile's version or draft runs on.

use std::sync::Arc;

use orchester_core::{
  connection::{NativeModel, NativeModelsError, ensure_used_kept},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, ModelId, Pipeline, ResourceKind, StepKind},
  run::StepExecutor,
  store::{ResourceStore, RunStore, TaskStore},
};
use orchester_store::{ProjectDb, StoreError};

use crate::{messages::Message, tasks::detail::run_store_error};

/// Who runs on a model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelUser {
  /// A profile, by its role: its published version or its draft.
  Profile(Arc<str>),
  /// A published pipeline step whose own model overrides its profile's.
  Step { pipeline: Arc<str>, step: Arc<str> },
}

/// A model of `connection` that `user` runs on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelUse {
  pub connection: ConnectionId,
  pub user: ModelUser,
  pub model: ModelId,
}

/// Every model use in the project, each listed once.
pub fn load(db: &ProjectDb) -> Result<Vec<ModelUse>, StoreError> {
  let mut uses = Vec::new();
  // The connections each profile runs on, in its published version and in its draft.
  let mut on: Vec<(ResourceId, ConnectionId)> = Vec::new();
  for resource in db.resources(ResourceKind::AgentProfile)? {
    let latest = db.latest_version::<AgentProfile>(resource.id())?;
    let draft = db.draft::<AgentProfile>(resource.id())?;
    let bodies = latest
      .as_ref()
      .map(|version| version.body())
      .into_iter()
      .chain(draft.as_ref().map(|draft| draft.body()));
    for profile in bodies {
      on.push((resource.id(), profile.connection()));
      push(&mut uses, profile_use(profile, profile.model().clone()));
    }
  }
  pinned_uses(db, &mut uses)?;
  for resource in db.resources(ResourceKind::Pipeline)? {
    let latest = db.latest_version::<Pipeline>(resource.id())?;
    let draft = db.draft::<Pipeline>(resource.id())?;
    let bodies = latest
      .as_ref()
      .map(|version| version.body())
      .into_iter()
      .chain(draft.as_ref().map(|draft| draft.body()));
    for pipeline in bodies {
      for step in pipeline.steps() {
        let StepKind::Agent {
          profile: Some(profile),
          model: Some(model),
          ..
        } = &step.kind
        else {
          continue;
        };
        for (_, connection) in on.iter().filter(|(on, _)| on == profile) {
          let user = ModelUser::Step {
            pipeline: resource.name().as_str().into(),
            step: step.name.as_str().into(),
          };
          push(
            &mut uses,
            ModelUse {
              connection: *connection,
              user,
              model: model.clone(),
            },
          );
        }
      }
    }
  }
  Ok(uses)
}

/// The models the steps of unfinished runs (running, waiting, paused or interrupted) were pinned to,
/// on their pinned profile versions' connections: such a run goes on with them. An unreadable run is
/// left out; the task list says it cannot be read.
fn pinned_uses(db: &ProjectDb, uses: &mut Vec<ModelUse>) -> Result<(), StoreError> {
  for task in db.tasks()? {
    for run in db
      .runs_of_task(task.id())
      .map_err(run_store_error)?
      .into_iter()
      .flatten()
    {
      if run.status().is_terminal() {
        continue;
      }
      for step in run.snapshot().steps() {
        let StepExecutor::Agent { profile, model, .. } = step.executor() else {
          continue;
        };
        if let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)? {
          push(uses, profile_use(version.body(), model.clone()));
        }
      }
    }
  }
  Ok(())
}

fn profile_use(profile: &AgentProfile, model: ModelId) -> ModelUse {
  ModelUse {
    connection: profile.connection(),
    user: ModelUser::Profile(profile.role().as_str().into()),
    model,
  }
}

fn push(uses: &mut Vec<ModelUse>, used: ModelUse) {
  if !uses.contains(&used) {
    uses.push(used);
  }
}

/// The uses among `uses` of models of `connection`.
pub fn of_connection(uses: &[ModelUse], connection: ConnectionId) -> Vec<ModelUse> {
  uses
    .iter()
    .filter(|used| used.connection == connection)
    .cloned()
    .collect()
}

/// The keys that say a model is still used: by a profile (`{{profile}}`) or by a pipeline step
/// (`{{pipeline}}`, `{{step}}`), with `{{model}}`.
#[derive(Clone, Copy, Debug)]
pub struct UsedKeys {
  pub profile: &'static str,
  pub step: &'static str,
}

/// Refuses `after` while something in `uses` runs on a model `before` listed and `after` no longer
/// does (core's [`ensure_used_kept`]), naming the first such user and its model.
pub fn keep_used(
  before: &[NativeModel],
  after: &[NativeModel],
  uses: &[ModelUse],
  keys: UsedKeys,
) -> Result<(), Message> {
  let Err(NativeModelsError::UsedModelDropped(model)) =
    ensure_used_kept(before, after, uses.iter().map(|used| &used.model))
  else {
    return Ok(());
  };
  let user = &uses
    .iter()
    .find(|used| used.model == model)
    .expect("the dropped model is one of the uses")
    .user;
  let message = match user {
    ModelUser::Step { pipeline, step } => Message::new(keys.step)
      .with("pipeline", pipeline.to_string())
      .with("step", step.to_string()),
    ModelUser::Profile(role) => Message::new(keys.profile).with("profile", role.to_string()),
  };
  Err(message.with("model", model.as_str().to_owned()))
}
