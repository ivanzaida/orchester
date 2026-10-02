//! Who runs on which model of an agent connection, for the rule that such a model stays listed (core's
//! [`ensure_used_listed`]): the latest published version of every profile and its draft, each on its
//! own connection and model, and every step of a latest published pipeline whose own model overrides
//! its profile's, on each connection that profile's version or draft runs on.

use std::sync::Arc;

use orchester_core::{
  connection::{NativeModel, NativeModelsError, ensure_used_listed},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, ModelId, ResourceKind},
  store::ResourceStore,
};

use crate::{agents::data::step_uses, messages::Message};

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
pub fn load<S: ResourceStore>(store: &S) -> Result<Vec<ModelUse>, S::Error> {
  let mut uses = Vec::new();
  // The connections each profile runs on, in its published version and in its draft.
  let mut on: Vec<(ResourceId, ConnectionId)> = Vec::new();
  for resource in store.resources(ResourceKind::AgentProfile)? {
    let latest = store.latest_version::<AgentProfile>(resource.id())?;
    let draft = store.draft::<AgentProfile>(resource.id())?;
    let bodies = latest
      .as_ref()
      .map(|version| version.body())
      .into_iter()
      .chain(draft.as_ref().map(|draft| draft.body()));
    for profile in bodies {
      on.push((resource.id(), profile.connection()));
      push(
        &mut uses,
        ModelUse {
          connection: profile.connection(),
          user: ModelUser::Profile(profile.role().as_str().into()),
          model: profile.model().clone(),
        },
      );
    }
  }
  for step in step_uses(store)? {
    let Some(model) = step.model_override else {
      continue;
    };
    for (_, connection) in on.iter().filter(|(profile, _)| *profile == step.profile) {
      push(
        &mut uses,
        ModelUse {
          connection: *connection,
          user: ModelUser::Step {
            pipeline: step.pipeline.clone(),
            step: step.step.clone(),
          },
          model: model.clone(),
        },
      );
    }
  }
  Ok(uses)
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

/// Refuses `models` while something in `uses` runs on a model they no longer list (core's
/// [`ensure_used_listed`]), naming the first such user and its model.
pub fn keep_used(models: &[NativeModel], uses: &[ModelUse], keys: UsedKeys) -> Result<(), Message> {
  let Err(NativeModelsError::UsedModelDropped(model)) = ensure_used_listed(models, uses.iter().map(|used| &used.model))
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
